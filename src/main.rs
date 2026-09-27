mod config;
mod decoder;
mod decryptor;
mod extractor;
mod network;
mod proto;
mod utils;

use std::path::Path;
use std::path::PathBuf;

use anyhow::{Context, Result};
use indexmap::IndexMap;
use unicode_width::UnicodeWidthStr;
use file_utils::FileHandler;
use clap::Parser;
use decoder::TDecodedManifestIndexMap;
use decoder::ManifestDecoder;
use decryptor::AeadTool;
use network::ManifestFetcher;
use extractor::HotfixPatchManifestExtractor;
use extractor::PatchVersionExtractor;
use extractor::GameVersionExtractor;
use network::ResourcesFetcher;
use utils::HotfixPatchMerger;

use crate::config::AppConfig;
use extractor::TManifestEntryIndexMap;

pub type TManifestIndexMap = IndexMap<String, Vec<u8>>;
pub type TManifestResult = Result<TManifestIndexMap>;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct ArgOptions {
    /// 生成清单记录
    #[arg(short = 'g', long, default_value_t = false)]
    pub generate_manifest_record: bool,
}

/// 请求服务器获取资源清单原始响应体数据
async fn get_raw_manifests() -> TManifestResult {
    let fetcher: ManifestFetcher = ManifestFetcher::new();
    fetcher
        .fetch_all_manifest()
        .await
        .context("failed to fetch all manifests")
}

/// 使用 AES-CBC-DECRYPT 模式解密原始清单数据体
fn decrypt_manifests(manifest_data: TManifestIndexMap) -> TManifestResult {
    let app_config: &AppConfig = AppConfig::get();
    let mut result: TManifestIndexMap = IndexMap::with_capacity(manifest_data.len());
    for (region, value) in manifest_data {
        logger::head!("DECRYPT MANIFEST {}", region);
        let decrypted: Vec<u8> = AeadTool::decrypt_cbc(&region, &value)
            .with_context(|| format!("decrypt failed for region '{region}'"))?;

        if app_config.feature_flags.is_save_decrypted_manifest {
            let save_path = Path::new(&app_config.file_path.manifest_output_dir)
                .join(&region)
                .join(&app_config.file_name.manifest_decrypt_file);
            FileHandler::write_bytes(&decrypted, &save_path)?;
            log::info!("[{region}] Saved path= {}", save_path.display());
        }

        result.insert(region, decrypted);
    }

    Ok(result)
}


/// 深度合并用于版本数据对象合并成一个 JSON
fn deep_merge(base: &mut serde_json::Value, overlay: serde_json::Map<String, serde_json::Value>) {
    if let Some(obj) = base.as_object_mut() {
        for (key, val) in overlay {
            if let Some(existing) = obj.get_mut(&key) {
                if let Some(inner) = existing.as_object_mut() {
                    if let serde_json::Value::Object(fields) = val {
                        for (k, v) in fields {
                            inner.insert(k, v);
                        }
                    }
                }
            } else {
                obj.insert(key, val);
            }
        }
    }
}

/// 保存提取的版本 JSON 数据文件
fn save_version_json(version_data: serde_json::Value) -> Result<()> {
    let app_config: &AppConfig = AppConfig::get();
    let save_path: PathBuf = Path::new(&app_config.file_path.versions_output_dir)
        .join(&app_config.file_name.version_file);
    FileHandler::write_json(&version_data, &save_path)?;
    log::info!("saved version json path= {}", save_path.display());

    Ok(())
}

fn print_boxed(lines: &[&str]) {
    // let is_ci: bool = std::env::var("CI").is_ok();
    let max_width: usize = lines.iter()
        .map(|l| l.width())
        .max()
        .unwrap_or(0);
    let inner_width: usize = max_width + 2;

    logger::tips!("{}", format!("╔{}╗", "═".repeat(inner_width)));
    for line in lines {
        let pad: usize = inner_width - line.width() - 2;
        logger::tips!("║ {}{} ║", line, " ".repeat(pad));
    }
    logger::tips!("{}", format!("╚{}╝", "═".repeat(inner_width)));
}


#[tokio::main]
async fn main() -> Result<()> {
    logger::init_logger!();

    let args: ArgOptions = ArgOptions::parse();
    // logger::tips!("StellaSora");
    // logger::tips!(" (╯°□°)╯");
    
    print_boxed(&[
        format!("             StellaSora   v{}    ResourceFetcher         ", env!("CARGO_PKG_VERSION")).as_str(),
        "               (╯°□°)╯     ₍^..^₎      ╰(°□°╰)             ",
    ]);

    let app_config: &AppConfig = AppConfig::get();

    let raw_manifests: TManifestIndexMap = get_raw_manifests().await?;
    let decrypted_manifests: TManifestIndexMap = decrypt_manifests(raw_manifests)?;

    let manifest_decoder: ManifestDecoder = ManifestDecoder::new(
        app_config.file_path.manifest_output_dir.clone(),
        app_config.file_name.manifest_decoded_file.clone(),
    );

    let decoder_result: TDecodedManifestIndexMap = manifest_decoder
        .decode_and_save_all(&decrypted_manifests)?;

    if !app_config.feature_flags.is_extract_patch_manifest {
        logger::tips!("FEATURE: IS_EXTRACT_PATCH_MANIFEST IS DISABLED.");
        return Ok(());}

    let patch_manifest_extractor: HotfixPatchManifestExtractor = HotfixPatchManifestExtractor::new();
    let all_region_patches: TManifestEntryIndexMap = if !args.generate_manifest_record {
        patch_manifest_extractor.extract_from_data(&decoder_result)?
    } else {
        patch_manifest_extractor.extract_root_manifest_from_data(&decoder_result)?
    };
    
    if !app_config.feature_flags.is_download_resource {
        logger::tips!("FEATURE: IS_DOWNLOAD_RESOURCE IS DISABLED.");
        return Ok(());
    }

    let resource_fetcher: ResourcesFetcher = ResourcesFetcher::new();
    resource_fetcher.fetch_and_save_from_manifest(&all_region_patches).await?;

    if !app_config.feature_flags.is_merge_patch || args.generate_manifest_record {
        logger::tips!("FEATURE: IS_MERGE_PATCH IS DISABLED.");
    } else {
        HotfixPatchMerger::new().apply_all_patches()?;
    }

    if app_config.feature_flags.is_save_version_file {
        let version_json: serde_json::Value = if app_config.feature_flags.is_extract_patch_version 
            && !app_config.feature_flags.is_use_resource_regex 
        {
            let version_extractor: PatchVersionExtractor<'_> = PatchVersionExtractor::new(&all_region_patches);
            if !args.generate_manifest_record {
                version_extractor.extract(app_config.resource_registry.unpack_set())?
            } else {
                version_extractor.extract_mani(app_config.file_name.update_root_mani_file.clone().as_str())?
            }
        } else {
            serde_json::json!({})
        };
        
        let game_version_extractor: GameVersionExtractor = GameVersionExtractor::new();
        let game_version_json: serde_json::Map<String, serde_json::Value>  = game_version_extractor.extract_regions()?;
        
        let mut version_merged_json: serde_json::Value = version_json;
        deep_merge(&mut version_merged_json, game_version_json);
        save_version_json(version_merged_json)?;
    }

    Ok(())
}

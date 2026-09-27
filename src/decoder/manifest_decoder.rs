use std::path::PathBuf;

use anyhow::Context;
use anyhow::Result;
use indexmap::IndexMap;
use prost::Message;

use file_utils::FileHandler;

use crate::config::AppConfig;
use crate::proto::FileDiff;
use crate::proto::PbClientDiff;

type TManifestDecodedVec = Vec<FileDiff>;
type TDecodedDataVec = Option<TManifestDecodedVec>;
type TDecodedResultVec = Result<TDecodedDataVec>;
pub type TDecodedManifestIndexMap = IndexMap<String, Vec<FileDiff>>;
type TDecodedResultMap = Result<TDecodedManifestIndexMap>;

pub struct ManifestDecoder {
    output_dir: PathBuf,
    output_filename: String,
    app_config: &'static AppConfig,
}

impl ManifestDecoder {
    pub fn new(output_dir: impl Into<PathBuf>, output_filename: impl Into<String>) -> Self {
        let app_config: &AppConfig = AppConfig::get();
        Self {
            output_dir: output_dir.into(),
            output_filename: output_filename.into(),
            app_config,
        }
    }

    pub fn decode_manifest(region: &str, data: &[u8]) -> TDecodedResultVec {
        if data.is_empty() {
            anyhow::bail!("[{region}] decrypted manifest data is empty");
        }
        match PbClientDiff::decode(data).context("failed to decode Pb_ClientDiff protobuf") {
            Ok(msg) => {
                let items: Vec<FileDiff> = msg.file_diff;
                log::debug!(
                    "[{region}] decode manifest successfully, {} item(s)",
                    items.len()
                );
                Ok(Some(items))
            }
            Err(e) => {
                anyhow::bail!("[{region}] failed to decode manifest: {e:#}");
            }
        }
    }

    pub fn save_decoded_manifest(
        &self,
        region: &str,
        data: Option<&TManifestDecodedVec>,
    ) -> Result<()> {
        let Some(items) = data else {
            log::warn!("[{}] no decoded manifest data to save", region);
            return Ok(());
        };

        let map: IndexMap<&str, &FileDiff> = items
            .iter()
            .filter_map(|item| {
                let name: &str = item.file_name.as_str();
                if name.is_empty() {
                    None
                } else {
                    Some((name, item))
                }
            })
            .collect();

        let save_path: PathBuf = self.output_dir.join(region).join(&self.output_filename);
        FileHandler::write_json(&map, &save_path)?;

        log::info!("[{}] saved path= {}", region, save_path.display());

        Ok(())
    }

    pub fn decode_and_save_all(
        &self,
        decrypted_data: &IndexMap<String, Vec<u8>>,
    ) -> TDecodedResultMap {
        if decrypted_data.is_empty() {
            anyhow::bail!("no decrypted manifest data provided");
        }
        let mut result: TDecodedManifestIndexMap = IndexMap::with_capacity(decrypted_data.len());
        for (region, data) in decrypted_data {
            if data.is_empty() {
                anyhow::bail!(
                    "Failed: decrypted manifest data is empty for region={}",
                    region
                );
            }
            logger::head!("DECODE MANIFEST {}", region);
            let decoded_data: Option<Vec<FileDiff>> = Self::decode_manifest(region, data)?;
            if self.app_config.feature_flags.is_save_decoded_manifest {
                self.save_decoded_manifest(region, decoded_data.as_ref())?;
            };
            if let Some(file_diffs) = decoded_data {
                result.insert(region.clone(), file_diffs);
            }
        }

        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::TManifestIndexMap;
    use file_utils::FileHandler;
    use indexmap::IndexMap;
    use logger;
    use std::path::Path;

    #[tokio::test]
    async fn test_func() -> Result<()> {
        logger::init_logger!();

        let app_config: &AppConfig = AppConfig::get();
        let decoded_filename: String = app_config.file_name.manifest_decoded_file.clone();
        let decrypt_filename: String = app_config.file_name.manifest_decrypt_file.clone();

        let manifest_decoder: ManifestDecoder = ManifestDecoder::new("./test", decoded_filename);

        let mut map: TManifestIndexMap = IndexMap::new();
        for region in app_config.available_regions() {
            let save_path: std::path::PathBuf = FileHandler::resolve_path(
                Path::new("./test").join(&region).join(&decrypt_filename),
            )?;

            if let Some(raw_bytes) = FileHandler::read_bytes(save_path) {
                map.insert(region.to_string(), raw_bytes);
            }
        }

        assert!(!map.is_empty());

        manifest_decoder.decode_and_save_all(&map)?;

        Ok(())
    }
}

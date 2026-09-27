use std::io::Read;

use aes::Aes128;
use aes::cipher::BlockModeDecrypt;
use aes::cipher::KeyIvInit;
use aes::cipher::block_padding::Pkcs7;
use anyhow::Context;
use anyhow::Result;
use cbc::Decryptor;
use flate2::read::GzDecoder;

use crate::config::AppConfig;
use crate::config::ServerConfigOwned;

type TAes128CbcDec = Decryptor<Aes128>;

pub struct AeadTool;

impl AeadTool {
    pub fn decrypt_cbc(region: &str, data: &[u8]) -> Result<Vec<u8>> {
        let decompressed: Vec<u8> =
            Self::decompress(region, data).context(format!("decompress failed for {region}"))?;

        if decompressed.len() < 16 {
            anyhow::bail!("decompressed data shorter than IV size");
        }

        let (iv, ciphertext) = decompressed.split_at(16);
        let server_config: &ServerConfigOwned = AppConfig::get()
            .get_server(region)
            .ok_or_else(|| anyhow::anyhow!("unknown region '{region}'"))?;

        if !Self::is_valid_config(region, server_config) {
            anyhow::bail!("invalid config for region '{region}'");
        }

        let aes_key: &[u8] = server_config.key.as_bytes();

        if aes_key.len() != 16 {
            anyhow::bail!(
                "AES-128 key must be 16 bytes, got {} (region='{region}')",
                aes_key.len()
            );
        }

        if iv.len() != 16 {
            anyhow::bail!("AES-CBC IV must be 16 bytes, got {}", iv.len());
        }

        log::debug!("AES key hex={}", hex::encode(aes_key));
        log::debug!("AES IV  hex={}", hex::encode(iv));

        log::debug!(
            "AES-128-CBC decrypt: key_len={}, iv_len={}, ciphertext_len={}",
            aes_key.len(),
            iv.len(),
            ciphertext.len()
        );

        let result: Vec<u8> = TAes128CbcDec::new_from_slices(aes_key, iv)
            .map_err(|e| anyhow::anyhow!("AES init error: {e}"))?
            .decrypt_padded_vec::<Pkcs7>(ciphertext)
            .map_err(|e| anyhow::anyhow!("AES decrypt/padding error: {e}"))?;

        Self::print_bytes_info(&result);

        Ok(result)
    }

    fn is_valid_config(region: &str, config: &ServerConfigOwned) -> bool {
        if config.url.is_empty() || config.key.is_empty() {
            log::warn!("invalid config for region={region}");
            return false;
        }
        true
    }

    fn try_gzip(data: &[u8]) -> Result<Vec<u8>> {
        let mut dec: GzDecoder<&[u8]> = GzDecoder::new(data);
        let mut buf: Vec<u8> = Vec::new();
        dec.read_to_end(&mut buf)?;

        Ok(buf)
    }

    fn decompress(region: &str, raw: &[u8]) -> Option<Vec<u8>> {
        let chunked: Vec<u8> = Self::decode_chunked(raw);
        let source: &[u8] = if chunked.is_empty() { raw } else { &chunked };
        match Self::try_gzip(source) {
            Ok(data) if data.len() >= 16 => {
                log::debug!("decompressed size={}", data.len());
                Some(data)
            }
            Ok(_) => {
                log::error!("decompressed data too short for region={region}");
                None
            }
            Err(e) => {
                log::debug!("gunzip failed for region={region}, err={e}, returning raw");
                Some(raw.to_vec())
            }
        }
    }

    fn decode_chunked(data: &[u8]) -> Vec<u8> {
        let mut out: Vec<u8> = Vec::new();
        let mut i: usize = 0;
        while i < data.len() {
            let end: usize = match data[i..].windows(2).position(|w| w == b"\r\n") {
                Some(p) => i + p,
                None => break,
            };
            let size: usize = std::str::from_utf8(&data[i..end])
                .ok()
                .and_then(|s| usize::from_str_radix(s.trim(), 16).ok())
                .unwrap_or(0);
            i = end + 2;
            if size == 0 || i + size > data.len() {
                break;
            }
            out.extend_from_slice(&data[i..i + size]);
            i += size + 2;
        }
        out
    }

    fn print_bytes_info(data: &[u8]) {
        if data.is_empty() {
            return;
        }
        let preview: &[u8] = &data[..std::cmp::min(32, data.len())];
        log::info!("hex={}", hex::encode(preview));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::TManifestIndexMap;
    use crate::TManifestResult;
    use file_utils::FileHandler;
    use indexmap::IndexMap;
    use std::path::Path;

    #[tokio::test]
    async fn test_func() -> Result<()> {
        logger::init_logger!();

        let app_config: &AppConfig = AppConfig::get();
        let raw_filename: String = app_config.file_name.manifest_raw_file.clone();
        let decrypted_filename = app_config.file_name.manifest_decrypt_file.clone();

        fn decrypt_manifests(manifest_data: TManifestIndexMap) -> TManifestResult {
            logger::head!("DECRYPT MANIFEST");
            let mut result: IndexMap<String, Vec<u8>> =
                IndexMap::with_capacity(manifest_data.len());
            for (region, value) in manifest_data {
                let decrypted: Vec<u8> = AeadTool::decrypt_cbc(&region, &value)
                    .with_context(|| format!("decrypt failed for region '{region}'"))?;
                result.insert(region, decrypted);
            }

            Ok(result)
        }

        let mut map: TManifestIndexMap = IndexMap::new();
        for region in app_config.available_regions() {
            let raw_path: std::path::PathBuf =
                FileHandler::resolve_path(Path::new("./test").join(region).join(&raw_filename))?;

            if let Some(raw_bytes) = FileHandler::read_bytes(raw_path) {
                map.insert(region.to_string(), raw_bytes);
            }
        }

        assert!(!map.is_empty());

        let decrypted_manifests: TManifestIndexMap = decrypt_manifests(map)?;

        for (region, decrypted_data) in decrypted_manifests {
            let decrypted_save_path: std::path::PathBuf = FileHandler::resolve_path(
                Path::new("./test").join(region).join(&decrypted_filename),
            )?;
            FileHandler::write_bytes(&decrypted_data, decrypted_save_path)?;
        }

        Ok(())
    }
}

use std::io::Read;

use aes::Aes128;
use aes::cipher::block_padding::Pkcs7;
use aes::cipher::{BlockModeDecrypt, KeyIvInit};
use cbc::Decryptor;
use flate2::read::GzDecoder;
use indexmap::IndexMap;

use crate::config::AppConfig;
use crate::error::{AppError, Result};
use crate::model::Region;

const KEY_LEN: usize = 16;
const IV_LEN: usize = 16;

type Aes128CbcDecryptor = Decryptor<Aes128>;

pub struct ManifestDecryptor {
    /// 各区域解密密钥
    keys: IndexMap<Region, String>,
}

impl ManifestDecryptor {
    pub fn new(keys: IndexMap<Region, String>) -> Self {
        Self { keys }
    }

    pub fn from_app_config(app: &AppConfig) -> Self {
        let keys = app
            .servers
            .iter()
            .map(|(name, server)| (Region::new(name), server.key.clone()))
            .collect();

        Self::new(keys)
    }

    pub fn decrypt_one(&self, region: &Region, data: &[u8]) -> Result<Vec<u8>> {
        logger::head!("DECRYPT MANIFEST {region}");

        let key = self
            .keys
            .get(region)
            .filter(|key| !key.is_empty())
            .ok_or_else(|| AppError::MissingKey(region.to_string()))?;

        let key_bytes = key.as_bytes();
        if key_bytes.len() != KEY_LEN {
            return Err(AppError::message(format!(
                "[{region}] AES-128 key must be {KEY_LEN} bytes, got {} bytes",
                key_bytes.len()
            )));
        }

        let payload = Self::decode_payload(region, data);
        if payload.len() < IV_LEN {
            return Err(AppError::message(format!(
                "[{region}] payload shorter than one IV block ({IV_LEN} bytes), got {} bytes",
                payload.len()
            )));
        }

        let (iv, ciphertext) = payload.split_at(IV_LEN);

        log::debug!(
            "[{region}] AES-128-CBC: key_len={}, iv_len={}, ciphertext_len={}",
            key_bytes.len(),
            iv.len(),
            ciphertext.len()
        );

        let decrypted = Aes128CbcDecryptor::new_from_slices(key_bytes, iv)
            .map_err(|err| AppError::message(format!("[{region}] AES init failed: {err}")))?
            .decrypt_padded_vec::<Pkcs7>(ciphertext)
            .map_err(|err| {
                AppError::message(format!("[{region}] AES decrypt/unpad failed: {err}"))
            })?;

        Self::log_preview(region, &decrypted);

        Ok(decrypted)
    }

    fn decode_payload(region: &Region, raw: &[u8]) -> Vec<u8> {
        let chunked = Self::decode_chunked(raw);
        let payload: &[u8] = chunked.as_deref().unwrap_or(raw);

        match Self::gunzip(payload) {
            Ok(data) => {
                log::debug!("[{region}] decompressed size={} bytes", data.len());
                data
            }
            Err(err) => {
                log::debug!("[{region}] gunzip failed, keeping raw payload: {err}");
                payload.to_vec()
            }
        }
    }

    fn gunzip(data: &[u8]) -> std::io::Result<Vec<u8>> {
        let mut decoder = GzDecoder::new(data);
        let mut buf = Vec::new();
        decoder.read_to_end(&mut buf)?;
        Ok(buf)
    }

    fn decode_chunked(data: &[u8]) -> Option<Vec<u8>> {
        let mut out = Vec::new();
        let mut cursor = 0;

        while cursor < data.len() {
            let offset = data[cursor..].windows(2).position(|w| w == b"\r\n")?;
            let size = Self::parse_chunk_size(&data[cursor..cursor + offset])?;
            cursor += offset + 2;

            if size == 0 {
                return Some(out);
            }

            let end = cursor.checked_add(size)?;
            if end > data.len() {
                return None;
            }

            out.extend_from_slice(&data[cursor..end]);
            cursor = end + 2;
        }

        Some(out)
    }

    fn parse_chunk_size(bytes: &[u8]) -> Option<usize> {
        let text = std::str::from_utf8(bytes).ok()?.trim();
        let hex_part = text.split(';').next().unwrap_or(text);

        usize::from_str_radix(hex_part, 16).ok()
    }

    fn log_preview(region: &Region, data: &[u8]) {
        if data.is_empty() {
            log::warn!("[{region}] decryption produced an empty result");
            return;
        }

        let preview_len = std::cmp::min(32, data.len());
        log::info!("[{region}] hex={}", hex::encode(&data[..preview_len]));
    }
}

#[cfg(test)]
mod tests {
    use super::ManifestDecryptor;

    fn chunked_of(chunks: &[&[u8]]) -> Vec<u8> {
        let mut out = Vec::new();
        for chunk in chunks {
            out.extend_from_slice(format!("{:x}\r\n", chunk.len()).as_bytes());
            out.extend_from_slice(chunk);
            out.extend_from_slice(b"\r\n");
        }
        out.extend_from_slice(b"0\r\n\r\n");
        out
    }

    #[test]
    fn decodes_chunked_payload() {
        let data = chunked_of(&[b"abc", b"de", b"fghij"]);
        assert_eq!(
            ManifestDecryptor::decode_chunked(&data),
            Some(b"abcdefghij".to_vec())
        );
    }

    #[test]
    fn decodes_single_chunk() {
        let data = chunked_of(&[b"hello world"]);
        assert_eq!(
            ManifestDecryptor::decode_chunked(&data),
            Some(b"hello world".to_vec())
        );
    }

    #[test]
    fn accepts_chunk_extension() {
        let mut data = Vec::new();
        data.extend_from_slice(b"3;name=value\r\nabc\r\n0\r\n\r\n");
        assert_eq!(
            ManifestDecryptor::decode_chunked(&data),
            Some(b"abc".to_vec())
        );
    }

    #[test]
    fn rejects_non_chunked_binary() {
        assert_eq!(
            ManifestDecryptor::decode_chunked(b"\x1f\x8b\x08\x00abcdef"),
            None
        );
        assert_eq!(
            ManifestDecryptor::decode_chunked(b"not-a-hex-size\r\nabc\r\n"),
            None
        );
        assert_eq!(ManifestDecryptor::decode_chunked(b""), Some(Vec::new()));
    }

    #[test]
    fn rejects_truncated_chunk() {
        assert_eq!(ManifestDecryptor::decode_chunked(b"a\r\nabc\r\n"), None);
    }
}

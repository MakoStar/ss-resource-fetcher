use anyhow::Context;
use indexmap::IndexMap;
use prost::Message;

use crate::error::{AppError, Result};
use crate::generated::{FileDiff, PbClientDiff};
use crate::model::Region;

pub struct ManifestDecoder;

impl ManifestDecoder {
    pub fn decode_one(region: &Region, data: &[u8]) -> Result<Vec<FileDiff>> {
        if data.is_empty() {
            return Err(AppError::EmptyData {
                context: format!("[{region}] decrypted manifest"),
            });
        }

        let message = PbClientDiff::decode(data)
            .with_context(|| format!("[{region}] protobuf decode failed"))?;
        let diffs = message.file_diff;

        log::debug!("[{region}] decoded {} entries", diffs.len());

        Ok(diffs)
    }

    pub fn json_view(diffs: &[FileDiff]) -> IndexMap<&str, &FileDiff> {
        diffs
            .iter()
            .filter(|diff| !diff.file_name.is_empty())
            .map(|diff| (diff.file_name.as_str(), diff))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use prost::Message;

    use super::ManifestDecoder;
    use crate::error::AppError;
    use crate::generated::{FileDiff, PbClientDiff};
    use crate::model::Region;

    fn encode_diffs(diffs: Vec<FileDiff>) -> Vec<u8> {
        PbClientDiff { file_diff: diffs }.encode_to_vec()
    }

    fn diff(name: &str, hash: &str, version: i64) -> FileDiff {
        FileDiff {
            file_name: name.to_string(),
            hash: hash.to_string(),
            version,
            additional_path: String::new(),
        }
    }

    #[test]
    fn roundtrips_encoded_diffs() {
        let diffs = vec![
            diff("char_2d_10801.unity3d", &"a".repeat(32), 133),
            diff("icon-0.unity3d", &"b".repeat(32), 133),
        ];

        let decoded =
            ManifestDecoder::decode_one(&Region::new("CN"), &encode_diffs(diffs.clone())).unwrap();

        assert_eq!(decoded, diffs);
    }

    #[test]
    fn decodes_single_entry_message() {
        let diffs = vec![diff("icon-0.unity3d", &"d".repeat(32), 133)];

        let decoded =
            ManifestDecoder::decode_one(&Region::new("CN"), &encode_diffs(diffs.clone())).unwrap();

        assert_eq!(decoded, diffs);
    }

    #[test]
    fn empty_bytes_report_as_empty_data() {
        let err = ManifestDecoder::decode_one(&Region::new("CN"), b"").unwrap_err();

        assert!(
            matches!(err, AppError::EmptyData { .. }),
            "expected EmptyData, got {err:?}"
        );
    }

    #[test]
    fn rejects_empty_payload() {
        let err = ManifestDecoder::decode_one(&Region::new("CN"), b"").unwrap_err();

        assert!(
            matches!(err, AppError::EmptyData { .. }),
            "expected EmptyData, got {err:?}"
        );
    }

    #[test]
    fn rejects_corrupted_protobuf() {
        let mut data = encode_diffs(vec![diff("x.unity3d", &"c".repeat(32), 1)]);
        data.truncate(data.len() / 2);

        let err = ManifestDecoder::decode_one(&Region::new("CN"), &data).unwrap_err();

        assert!(err.to_string().contains("protobuf decode failed"), "{err}");
    }

    #[test]
    fn json_view_skips_blank_names() {
        let diffs = vec![
            diff("", &"0".repeat(32), 1),
            diff("real.unity3d", &"1".repeat(32), 1),
        ];

        let view = ManifestDecoder::json_view(&diffs);

        assert_eq!(view.len(), 1);
        assert!(view.contains_key("real.unity3d"));
        assert!(!view.contains_key(""));
    }
}

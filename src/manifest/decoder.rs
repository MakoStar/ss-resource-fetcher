use anyhow::Context;
use indexmap::IndexMap;
use prost::Message;

use crate::error::{AppError, Result};
use crate::generated::{FileDiff, PbClientDiff};
use crate::model::{Region, RegionBytes, RegionFileDiffs};

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

    pub fn decode_all(decrypted: &RegionBytes) -> Result<RegionFileDiffs> {
        if decrypted.is_empty() {
            return Err(AppError::EmptyData {
                context: "decrypted manifest".into(),
            });
        }

        let mut decoded = IndexMap::with_capacity(decrypted.len());
        for (region, data) in decrypted {
            logger::head!("DECODE MANIFEST {region}");
            decoded.insert(region.clone(), Self::decode_one(region, data)?);
        }

        Ok(decoded)
    }

    pub fn json_view(diffs: &[FileDiff]) -> IndexMap<&str, &FileDiff> {
        diffs
            .iter()
            .filter(|diff| !diff.file_name.is_empty())
            .map(|diff| (diff.file_name.as_str(), diff))
            .collect()
    }
}

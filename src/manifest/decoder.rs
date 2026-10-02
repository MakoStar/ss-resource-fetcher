use anyhow::Context;
use indexmap::IndexMap;
use prost::Message;

use crate::error::{AppError, Result};
use crate::generated::{FileDiff, PbClientDiff};
use crate::model::Region;

pub struct ManifestDecoder;

impl ManifestDecoder {
    pub fn decode_one(region: &Region, data: &[u8]) -> Result<Vec<FileDiff>> {
        logger::head!("DECODE MANIFEST {region}");

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

use std::collections::HashMap;

use serde::Serialize;

pub enum RequestBody {
    None,
    Json(serde_json::Value),
    Form(Vec<(String, String)>),
    Bytes(Vec<u8>),
    Text(String),
}

impl RequestBody {
    pub fn json<T: Serialize>(value: &T) -> FetchResult<Self> {
        let v: serde_json::Value =
            serde_json::to_value(value).map_err(|e| FetchError::JsonSerialize { source: e })?;
        Ok(Self::Json(v))
    }

    pub fn form(map: &HashMap<String, String>) -> Self {
        Self::Form(map.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
    }
}

use crate::error::{FetchError, FetchResult};

use std::collections::HashMap;

use serde::Serialize;

#[derive(Debug)]
pub enum RequestBody {
    None,
    Json(serde_json::Value),
    Form(Vec<(String, String)>),
    Bytes(Vec<u8>),
    Text(String),
}

impl RequestBody {
    pub fn json<T: Serialize>(value: &T) -> FetchResult<Self> {
        let v = serde_json::to_value(value).map_err(|e| FetchError::JsonSerialize { source: e })?;
        Ok(Self::Json(v))
    }

    pub fn form(map: &HashMap<String, String>) -> Self {
        Self::Form(map.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
    }
}

use crate::error::{FetchError, FetchResult};

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::RequestBody;

    #[derive(serde::Serialize)]
    struct Payload {
        name: String,
        count: u32,
    }

    #[test]
    fn json_body_serializes_struct() {
        let body = RequestBody::json(&Payload {
            name: "stella".into(),
            count: 3,
        })
        .unwrap();

        match body {
            RequestBody::Json(value) => {
                assert_eq!(value["name"], "stella");
                assert_eq!(value["count"], 3);
            }
            other => panic!("expected Json body, got {other:?}"),
        }
    }

    #[test]
    fn form_body_converts_map_to_pairs() {
        let mut map = HashMap::new();
        map.insert("a".to_string(), "1".to_string());
        map.insert("b".to_string(), "2".to_string());

        let body = RequestBody::form(&map);

        match body {
            RequestBody::Form(pairs) => {
                assert_eq!(pairs.len(), 2);
                assert!(pairs.contains(&("a".to_string(), "1".to_string())));
            }
            other => panic!("expected Form body, got {other:?}"),
        }
    }
}

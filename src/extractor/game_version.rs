use std::path::PathBuf;

use serde_json::{Map, Value};

use crate::config::AppConfig;
use crate::error::Result;
use crate::manifest::ManiReader;
use crate::model::Region;

pub struct GameVersionExtractor {
    default_version_key: String,
    client_version_key: String,
    game_version_key: String,
    patch_version_key: String,
    patch_metadata_dir: PathBuf,
    root_manifest_filename: String,
    regions: Vec<Region>,
}

impl GameVersionExtractor {
    pub fn from_app_config(app: &AppConfig) -> Self {
        Self {
            default_version_key: app.extractor.default_version_key.clone(),
            client_version_key: app.extractor.client_version_key.clone(),
            game_version_key: app.extractor.game_version_key.clone(),
            patch_version_key: app.extractor.patch_version_key.clone(),
            patch_metadata_dir: PathBuf::from(&app.file_path.patch_output_dir),
            root_manifest_filename: app.file_name.update_root_mani_file.clone(),
            regions: app.regions(),
        }
    }

    fn manifest_patch_path(&self, region: &Region) -> PathBuf {
        self.patch_metadata_dir
            .join(region.as_str())
            .join(&self.root_manifest_filename)
    }

    pub fn extract_regions(&self) -> Result<Map<String, Value>> {
        logger::head!("EXTRACT CLIENT VERSION");

        let mut result = Map::with_capacity(self.regions.len());

        for region in &self.regions {
            let reader = ManiReader::new(self.manifest_patch_path(region))?;

            let client_ver = self.config_value(&reader, &self.client_version_key);
            let game_ver = self.config_value(&reader, &self.game_version_key);
            let patch_ver = self.config_value(&reader, &self.patch_version_key);

            log::debug!(
                "[{region}] CLIENT_VER={client_ver}, GAME_VER={game_ver}, BIN_DIFF_PATCH_VER={patch_ver}",
            );

            let version = format!("[c{client_ver}_g{game_ver}_p{patch_ver}]");

            let mut inner = Map::new();
            inner.insert("game_ver".to_string(), Value::String(game_ver.to_string()));
            inner.insert(
                "client_ver".to_string(),
                Value::String(client_ver.to_string()),
            );
            inner.insert(
                "patch_num".to_string(),
                Value::String(patch_ver.to_string()),
            );
            inner.insert("version_str".to_string(), Value::String(version));

            result.insert(region.to_string(), Value::Object(inner));
        }

        Ok(result)
    }

    fn config_value<'r>(&'r self, reader: &'r ManiReader, key: &str) -> &'r str {
        reader
            .get_config_value_by_key(key)
            .unwrap_or(&self.default_version_key)
    }
}

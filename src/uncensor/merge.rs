use std::path::{Path, PathBuf};

use file_utils::FileHandler;

use crate::config::ClientRegistryConfig;
use crate::error::{AppError, Result};
use crate::merger::apply_bsdiff;
use crate::model::Region;

pub struct UncensorMerger {
    /// 资源注册表 用于判断补丁类型
    registry: ClientRegistryConfig,
}

impl UncensorMerger {
    pub fn new(registry: ClientRegistryConfig) -> Self {
        Self { registry }
    }

    pub fn merge(
        &self,
        region: &Region,
        file: &str,
        base: Option<&Path>,
        steps: &[(&str, PathBuf)],
        output: &Path,
    ) -> Result<()> {
        if steps.is_empty() {
            let Some(base) = base else {
                return Err(AppError::message(format!(
                    "[{region}] {file} has no source file"
                )));
            };

            FileHandler::copy_file(base, output)?;
            return Ok(());
        }

        let mut current = base.map(Path::to_path_buf);

        for (name, path) in steps {
            if self.registry.is_new_file_patch(name) {
                log::debug!("[{region}] {file} replaced by {name}");
                FileHandler::copy_file(path, output)?;
                current = Some(output.to_path_buf());
                continue;
            }

            let Some(source) = &current else {
                return Err(AppError::message(format!(
                    "[{region}] {file} delta patch {name} has no base file"
                )));
            };

            let base_data = std::fs::read(source)?;
            let patch_data = std::fs::read(path)?;
            let merged = apply_bsdiff(&base_data, &patch_data, name)?;

            FileHandler::write_bytes(&merged, output)?;
            log::debug!("[{region}] {file} merged {name}");
            current = Some(output.to_path_buf());
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::UncensorMerger;
    use crate::config::AppConfig;
    use crate::model::Region;

    fn merger() -> UncensorMerger {
        UncensorMerger::new(AppConfig::default().task.client.registry)
    }

    #[test]
    fn copies_base_when_no_patch() {
        let dir = tempfile::tempdir().unwrap();
        let base = dir.path().join("base.bin");
        let output = dir.path().join("out.bin");
        std::fs::write(&base, b"original").unwrap();

        merger()
            .merge(&Region::new("CN"), "out.bin", Some(&base), &[], &output)
            .unwrap();

        assert_eq!(std::fs::read(&output).unwrap(), b"original");
    }

    #[test]
    fn fails_when_delta_has_no_base() {
        let dir = tempfile::tempdir().unwrap();
        let patch = dir.path().join("p_1_u.out.bin");
        let output = dir.path().join("out.bin");
        std::fs::write(&patch, b"patch").unwrap();

        let err = merger()
            .merge(
                &Region::new("CN"),
                "out.bin",
                None,
                &[("p_1_u.out.bin", patch)],
                &output,
            )
            .unwrap_err();

        assert!(err.to_string().contains("no base file"));
    }
}

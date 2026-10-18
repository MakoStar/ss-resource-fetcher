use serde::{Deserialize, Serialize};

use super::client::ClientTaskConfig;
use super::launcher::LauncherTaskConfig;
use super::uncensor::UncensorTaskConfig;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct TaskConfig {
    /// 热更客户端资源任务
    #[serde(rename = "CLIENT")]
    pub client: ClientTaskConfig,

    /// launcher 资源任务
    #[serde(rename = "LAUNCHER")]
    pub launcher: LauncherTaskConfig,

    /// 反和谐资源任务
    #[serde(rename = "UNCENSOR")]
    pub uncensor: UncensorTaskConfig,
}

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ServerRouteConfig {
    /// 清单文件的 API 路径
    #[serde(rename = "MANIFEST_ROUTE")]
    pub manifest_route: String,

    /// 资源文件的 API 路径
    #[serde(rename = "RESOURCE_ROUTE")]
    pub resource_route: String,
}

impl Default for ServerRouteConfig {
    fn default() -> Self {
        Self {
            manifest_route: "/meta/win.html".into(),
            resource_route: "/res/win/".into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    /// 服务器地址
    #[serde(rename = "URL")]
    pub url: String,

    /// 解密密钥
    #[serde(rename = "KEY")]
    pub key: String,
}

#[derive(Debug, Clone, Copy)]
pub struct StaticServer {
    /// 服务器地址
    pub url: &'static str,
    /// 解密密钥
    pub key: &'static str,
}

pub(crate) static STATIC_RESOURCE_SERVERS: phf::OrderedMap<&'static str, StaticServer> = phf::phf_ordered_map! {
    "CN" => StaticServer { url: "https://nova-static.yostar.cn", key: "Xf&FRcsYm48cJ2A@" },
    "EN" => StaticServer { url: "https://nova-static.stellasora.global", key: "ma5Dn2FhC*Xhxy%c" },
    "JP" => StaticServer { url: "https://nova-static.stellasora.jp", key: "ZnUFA@S9%4KyoryM" },
    "KR" => StaticServer { url: "https://nova-static.stellasora.kr", key: "U9cjHuwGDDx&$drn" },
    "TW" => StaticServer { url: "https://nova-static.stargazer-games.com", key: "owGYVDmfHrxi^4pm" },
};

impl From<&StaticServer> for ServerConfig {
    fn from(s: &StaticServer) -> Self {
        Self {
            url: s.url.into(),
            key: s.key.into(),
        }
    }
}

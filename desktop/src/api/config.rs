//! 配置 API(移植 frontend/src/api/config.ts)。AppConfig 复用后端类型。

use reqwest::Method;
use serde::Deserialize;
use serde_json::{Value, json};

use super::{ApiError, ApiClient};

pub use purepan_backend::config::AppConfig;

#[derive(Debug, Clone, Deserialize)]
pub struct ConfigUpdateResponse {
    pub message: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ProxyStatusResponse {
    pub status: serde_json::Value,
    pub flap_count: u32,
    pub next_probe_in_secs: Option<u64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ProxyTestResponse {
    pub success: bool,
    pub latency_ms: Option<u64>,
    pub error: Option<String>,
}

impl ApiClient {
    /// GET /config
    pub async fn get_config(&self) -> Result<AppConfig, ApiError> {
        self.request(Method::GET, "/config", None).await
    }

    /// PUT /config(整体 AppConfig)
    pub async fn update_config(&self, config: &AppConfig) -> Result<ConfigUpdateResponse, ApiError> {
        self.request(Method::PUT, "/config", Some(json!(config))).await
    }

    /// PUT /config(以 JSON Value 提交,便于按字段编辑)
    pub async fn update_config_value(&self, config: Value) -> Result<ConfigUpdateResponse, ApiError> {
        self.request(Method::PUT, "/config", Some(config)).await
    }

    /// POST /config/reset
    pub async fn reset_config(&self) -> Result<String, ApiError> {
        self.request(Method::POST, "/config/reset", Some(json!({}))).await
    }

    /// POST /config/recent-dir(dir_type: download/upload)
    pub async fn update_recent_dir(&self, dir_type: &str, path: &str) -> Result<String, ApiError> {
        self.request(
            Method::POST,
            "/config/recent-dir",
            Some(json!({ "dir_type": dir_type, "path": path })),
        )
        .await
    }

    /// POST /config/default-download-dir
    pub async fn set_default_download_dir(&self, path: &str) -> Result<String, ApiError> {
        self.request(
            Method::POST,
            "/config/default-download-dir",
            Some(json!({ "path": path })),
        )
        .await
    }

    /// GET /proxy/status
    pub async fn get_proxy_status(&self) -> Result<ProxyStatusResponse, ApiError> {
        self.request(Method::GET, "/proxy/status", None).await
    }

    /// POST /proxy/test
    pub async fn test_proxy(&self, proxy: &purepan_backend::common::ProxyConfig) -> Result<ProxyTestResponse, ApiError> {
        self.request(Method::POST, "/proxy/test", Some(json!(proxy))).await
    }
}
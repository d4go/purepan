//! 配置 API(进程内直调)。AppConfig 复用后端类型。

use serde::Deserialize;
use serde_json::Value;

use purepan_backend::server::inproc;

use super::{ApiClient, ApiError};

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
    pub async fn get_config(&self) -> Result<AppConfig, ApiError> {
        self.request(inproc::config::get).await
    }

    /// 以 JSON Value 提交,便于按字段编辑。
    pub async fn update_config_value(&self, config: Value) -> Result<ConfigUpdateResponse, ApiError> {
        self.request(move |s| inproc::config::update(s, config))
            .await
    }

    pub async fn get_proxy_status(&self) -> Result<ProxyStatusResponse, ApiError> {
        self.request(inproc::config::proxy_status).await
    }

    pub async fn test_proxy(&self, proxy: &purepan_backend::common::ProxyConfig) -> Result<ProxyTestResponse, ApiError> {
        let body = serde_json::to_value(proxy)
            .map_err(|e| ApiError::Decode(e.to_string()))?;
        self.request(move |s| inproc::config::test_proxy(s, body))
            .await
    }
}

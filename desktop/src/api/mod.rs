pub mod accounts;
pub mod auth;
pub mod autobackup;
pub mod cloud_dl;
pub mod config;
pub mod download;
pub mod dto;
pub mod file;
pub mod filesystem;
pub mod local_files;
pub mod share;
pub mod share_sync;
pub mod transfer;
pub mod upload;

use std::future::Future;

use purepan_backend::server::inproc::{self, InprocError};
use purepan_backend::server::AppState;
use serde::de::DeserializeOwned;
use tokio::sync::watch;

use crate::backend_embed::BackendStatus;

/// 视图侧便捷入口:在 tokio 上执行后端调用并把取消错误归一为 ApiError。
pub async fn call_api<F, Fut, T>(rt: &tokio::runtime::Handle, f: F) -> Result<T, ApiError>
where
    F: FnOnce() -> Fut + Send + 'static,
    Fut: std::future::Future<Output = Result<T, ApiError>> + Send + 'static,
    T: Send + 'static,
{
    crate::bridge::call(rt, f)
        .await
        .unwrap_or(Err(ApiError::Canceled))
}

#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    /// 业务错误(code != 0 或后端 ApiError),携带错误码供调用方分支(如转存 need-password)
    #[error("{message}")]
    Business { code: i64, message: String },
    #[error("{0}")]
    Network(String),
    #[error("响应解析失败: {0}")]
    Decode(String),
    #[error("请求已取消")]
    Canceled,
}

impl ApiError {
    pub fn message(&self) -> String {
        self.to_string()
    }
}

impl From<InprocError> for ApiError {
    fn from(e: InprocError) -> Self {
        ApiError::Business {
            code: e.code,
            message: e.message,
        }
    }
}

/// 进程内 API 客户端:直接调用后端 handler,不再经过 HTTP/令牌。
pub struct ApiClient {
    status: watch::Receiver<BackendStatus>,
}

impl ApiClient {
    pub fn new(status: watch::Receiver<BackendStatus>) -> Self {
        Self { status }
    }

    /// 等待后端初始化完成(失败时返回错误)。
    pub async fn state(&self) -> Result<AppState, ApiError> {
        let mut rx = self.status.clone();
        loop {
            let current = rx.borrow_and_update().clone();
            match current {
                BackendStatus::Ready(state) => return Ok(state),
                BackendStatus::Failed(message) => return Err(ApiError::Network(message)),
                BackendStatus::Starting => {
                    if rx.changed().await.is_err() {
                        return Err(ApiError::Network("后端已停止".into()));
                    }
                }
            }
        }
    }

    /// 在 tokio 运行时上执行一次进程内调用并反序列化为视图 DTO。
    pub(crate) async fn request<Fut, T>(&self, f: impl FnOnce(AppState) -> Fut) -> Result<T, ApiError>
    where
        Fut: Future<Output = Result<serde_json::Value, InprocError>>,
        T: DeserializeOwned,
    {
        let state = self.state().await?;
        let value = f(state).await.map_err(ApiError::from)?;
        serde_json::from_value(value).map_err(|e| ApiError::Decode(e.to_string()))
    }

    /// 下载外部图片(头像等),复用后端代理配置。
    pub async fn fetch_avatar(&self, url: &str) -> Result<Vec<u8>, ApiError> {
        let state = self.state().await?;
        inproc::avatar::fetch(&state, url.to_string())
            .await
            .map_err(ApiError::from)
    }
}

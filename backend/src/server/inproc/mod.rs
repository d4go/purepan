//! 进程内 API 门面
//!
//! 桌面端（GPUI）与后端运行在同一进程，通过本模块直接调用既有 handler，
//! 不再经过 HTTP/WebSocket。门面负责：
//! - 构造 axum 提取器并调用 handler；
//! - 解包各模块信封（`code/message/data` 或 `success/data/error`）；
//! - 把错误归一为 [`InprocError`]，保留原 HTTP 层对 UI 暴露的 code/message 语义。

use axum::http::StatusCode;
use serde::Serialize;

use crate::server::error::ApiError;

pub mod accounts;
pub mod auth;
pub mod autobackup;
pub mod avatar;
pub mod cloud_dl;
pub mod config;
pub mod download;
pub mod file;
pub mod filesystem;
pub mod local_files;
pub mod share;
pub mod share_sync;
pub mod transfer;
pub mod upload;

/// 进程内调用错误：`code` 与旧 HTTP 信封/状态码对齐，供视图分支（如 errno 132）。
#[derive(Debug, Clone, thiserror::Error)]
#[error("{message}")]
pub struct InprocError {
    pub code: i64,
    pub message: String,
}

impl InprocError {
    pub fn new(code: i64, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    pub fn status(status: StatusCode) -> Self {
        Self::new(
            status.as_u16() as i64,
            format!("请求失败 ({})", status.as_u16()),
        )
    }

    pub fn decode(e: impl std::fmt::Display) -> Self {
        Self::new(-1, format!("参数解析失败: {e}"))
    }

    pub fn missing_data() -> Self {
        Self::new(-1, "响应缺少 data 字段")
    }

    /// 与 `ApiError::into_response` 保持一致的 code/message。
    pub fn from_api_error(e: &ApiError) -> Self {
        match e {
            ApiError::Internal(err) => {
                tracing::error!("进程内调用内部错误: {err:#}");
                Self::new(500, "Internal server error")
            }
            ApiError::Unauthorized(m) => Self::new(401, m.clone()),
            ApiError::TokenExpired(m) => Self::new(401, m.clone()),
            ApiError::NotFound(m) => Self::new(404, m.clone()),
            ApiError::BadRequest(m) => Self::new(400, m.clone()),
            ApiError::Conflict(m) => Self::new(409, m.clone()),
            ApiError::AccountNotAvailable => Self::new(400, "account_not_available"),
            ApiError::LegacyHistoryReadonly => Self::new(400, "legacy_history_readonly"),
            ApiError::ReadonlyMode => Self::new(503, "readonly_mode"),
        }
    }
}

pub(crate) fn to_json<T: Serialize>(value: T) -> Result<serde_json::Value, InprocError> {
    serde_json::to_value(value).map_err(InprocError::decode)
}

/// 解包 `{code, message, data}` 信封（auth/download/upload/file/... 通用）。
pub(crate) fn envelope<T: Serialize>(
    code: i32,
    message: String,
    data: Option<T>,
) -> Result<serde_json::Value, InprocError> {
    if code != 0 {
        let message = if message.is_empty() {
            "请求失败".to_string()
        } else {
            message
        };
        return Err(InprocError::new(code as i64, message));
    }
    match data {
        Some(data) => to_json(data),
        None => Err(InprocError::missing_data()),
    }
}

/// 解包 `{success, data, error}` 信封（autobackup / share-sync 专用）。
pub(crate) fn success_envelope<T: Serialize>(
    success: bool,
    error: Option<String>,
    data: Option<T>,
) -> Result<serde_json::Value, InprocError> {
    if !success {
        return Err(InprocError::new(
            -1,
            error.unwrap_or_else(|| "操作失败".to_string()),
        ));
    }
    match data {
        Some(data) => to_json(data),
        None => Err(InprocError::missing_data()),
    }
}

pub(crate) fn parse_body<T: serde::de::DeserializeOwned>(
    body: serde_json::Value,
) -> Result<T, InprocError> {
    serde_json::from_value(body).map_err(InprocError::decode)
}

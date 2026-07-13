//! Web 认证错误处理模块
//!
//! 提供统一的错误类型定义

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Web 认证错误
#[derive(Debug, Error)]
pub enum WebAuthError {
    /// 密码不符合要求
    #[error("密码不符合要求: {0}")]
    InvalidPassword(String),

    /// 密码验证失败
    #[error("密码验证失败")]
    PasswordVerificationFailed,

    /// TOTP 验证失败
    #[error("TOTP 验证失败")]
    TotpVerificationFailed,

    /// 恢复码无效或已使用
    #[error("恢复码无效或已使用")]
    InvalidRecoveryCode,

    /// 令牌无效或已过期
    #[error("令牌无效或已过期")]
    InvalidToken,

    /// 刷新令牌无效或已过期
    #[error("刷新令牌无效或已过期")]
    InvalidRefreshToken,

    /// 请求过于频繁
    #[error("请求过于频繁，请在 {0} 秒后重试")]
    RateLimited(u64),

    /// 需要双因素认证
    #[error("需要双因素认证")]
    TotpRequired,

    /// 认证未启用
    #[error("认证未启用")]
    AuthNotEnabled,

    /// 配置错误
    #[error("配置错误: {0}")]
    ConfigError(String),

    /// 存储错误
    #[error("存储错误: {0}")]
    StorageError(String),

    /// 哈希错误
    #[error("哈希错误: {0}")]
    HashError(String),

    /// TOTP 错误
    #[error("TOTP 错误: {0}")]
    TotpError(String),

    /// 内部错误
    #[error("内部错误: {0}")]
    InternalError(String),
}

/// 错误响应结构（用于 API 返回）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorResponse {
    /// HTTP 状态码
    pub code: u16,
    /// 错误标识
    pub error: String,
    /// 错误消息
    pub message: String,
    /// 额外详情
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<ErrorDetails>,
}

/// 错误详情
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorDetails {
    /// 剩余锁定时间（秒）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lockout_remaining: Option<u64>,
}

impl WebAuthError {
    /// 转换为 HTTP 状态码
    pub fn status_code(&self) -> u16 {
        match self {
            WebAuthError::InvalidPassword(_) => 400,
            WebAuthError::PasswordVerificationFailed => 401,
            WebAuthError::TotpVerificationFailed => 401,
            WebAuthError::InvalidRecoveryCode => 401,
            WebAuthError::InvalidToken => 401,
            WebAuthError::InvalidRefreshToken => 401,
            WebAuthError::RateLimited(_) => 429,
            WebAuthError::TotpRequired => 403,
            WebAuthError::AuthNotEnabled => 400,
            WebAuthError::ConfigError(_) => 500,
            WebAuthError::StorageError(_) => 500,
            WebAuthError::HashError(_) => 500,
            WebAuthError::TotpError(_) => 500,
            WebAuthError::InternalError(_) => 500,
        }
    }

    /// 转换为错误标识
    pub fn error_code(&self) -> &'static str {
        match self {
            WebAuthError::InvalidPassword(_) => "invalid_password",
            WebAuthError::PasswordVerificationFailed => "password_verification_failed",
            WebAuthError::TotpVerificationFailed => "totp_verification_failed",
            WebAuthError::InvalidRecoveryCode => "invalid_recovery_code",
            WebAuthError::InvalidToken => "invalid_token",
            WebAuthError::InvalidRefreshToken => "invalid_refresh_token",
            WebAuthError::RateLimited(_) => "rate_limited",
            WebAuthError::TotpRequired => "totp_required",
            WebAuthError::AuthNotEnabled => "auth_not_enabled",
            WebAuthError::ConfigError(_) => "config_error",
            WebAuthError::StorageError(_) => "storage_error",
            WebAuthError::HashError(_) => "hash_error",
            WebAuthError::TotpError(_) => "totp_error",
            WebAuthError::InternalError(_) => "internal_error",
        }
    }

    /// 转换为错误响应
    pub fn to_response(&self) -> ErrorResponse {
        let details = match self {
            WebAuthError::RateLimited(remaining) => Some(ErrorDetails {
                lockout_remaining: Some(*remaining),
            }),
            _ => None,
        };

        ErrorResponse {
            code: self.status_code(),
            error: self.error_code().to_string(),
            message: self.to_string(),
            details,
        }
    }
}

impl From<std::io::Error> for WebAuthError {
    fn from(err: std::io::Error) -> Self {
        WebAuthError::StorageError(err.to_string())
    }
}

impl From<serde_json::Error> for WebAuthError {
    fn from(err: serde_json::Error) -> Self {
        WebAuthError::StorageError(err.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_status_codes() {
        assert_eq!(
            WebAuthError::InvalidPassword("too short".to_string()).status_code(),
            400
        );
        assert_eq!(WebAuthError::PasswordVerificationFailed.status_code(), 401);
        assert_eq!(WebAuthError::RateLimited(60).status_code(), 429);
        assert_eq!(
            WebAuthError::ConfigError("invalid".to_string()).status_code(),
            500
        );
    }

    #[test]
    fn test_error_response() {
        let err = WebAuthError::RateLimited(120);
        let response = err.to_response();

        assert_eq!(response.code, 429);
        assert_eq!(response.error, "rate_limited");
        assert!(response.details.is_some());
        assert_eq!(response.details.unwrap().lockout_remaining, Some(120));
    }
}

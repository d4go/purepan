//! Web 认证类型定义模块
//!
//! 定义认证配置、凭证存储等核心数据结构

use serde::{Deserialize, Serialize};

/// 认证模式
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum AuthMode {
    /// 无认证
    #[default]
    None,
    /// 仅密码
    Password,
    /// 仅 2FA (TOTP)
    Totp,
    /// 密码 + 2FA
    PasswordTotp,
}

impl AuthMode {
    /// 是否需要密码
    pub fn requires_password(&self) -> bool {
        matches!(self, AuthMode::Password | AuthMode::PasswordTotp)
    }

    /// 是否需要 TOTP
    pub fn requires_totp(&self) -> bool {
        matches!(self, AuthMode::Totp | AuthMode::PasswordTotp)
    }

    /// 是否启用了认证
    pub fn is_enabled(&self) -> bool {
        !matches!(self, AuthMode::None)
    }
}

/// Web 认证配置（存储在 app.toml）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebAuthConfig {
    /// 是否启用认证
    pub enabled: bool,
    /// 认证模式
    pub mode: AuthMode,
}

impl Default for WebAuthConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            mode: AuthMode::None,
        }
    }
}

/// 恢复码
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecoveryCode {
    /// 恢复码哈希
    pub code_hash: String,
    /// 是否已使用
    pub used: bool,
    /// 使用时间
    #[serde(skip_serializing_if = "Option::is_none")]
    pub used_at: Option<i64>,
}

impl RecoveryCode {
    /// 创建新的恢复码记录
    pub fn new(code_hash: String) -> Self {
        Self {
            code_hash,
            used: false,
            used_at: None,
        }
    }

    /// 标记为已使用
    pub fn mark_used(&mut self) {
        self.used = true;
        self.used_at = Some(chrono::Utc::now().timestamp());
    }
}

/// 认证凭证（存储在 auth.json）
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AuthCredentials {
    /// 密码哈希 (Argon2id)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub password_hash: Option<String>,

    /// TOTP 密钥 (Base32 编码)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub totp_secret: Option<String>,

    /// 恢复码列表 (哈希存储)
    #[serde(default)]
    pub recovery_codes: Vec<RecoveryCode>,

    /// 凭证更新时间
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<i64>,
}

impl AuthCredentials {
    /// 创建空凭证
    pub fn new() -> Self {
        Self::default()
    }

    /// 设置密码哈希
    pub fn set_password_hash(&mut self, hash: String) {
        self.password_hash = Some(hash);
        self.updated_at = Some(chrono::Utc::now().timestamp());
    }

    /// 清除密码
    pub fn clear_password(&mut self) {
        self.password_hash = None;
        self.updated_at = Some(chrono::Utc::now().timestamp());
    }

    /// 设置 TOTP 密钥
    pub fn set_totp_secret(&mut self, secret: String) {
        self.totp_secret = Some(secret);
        self.updated_at = Some(chrono::Utc::now().timestamp());
    }

    /// 清除 TOTP 密钥
    pub fn clear_totp(&mut self) {
        self.totp_secret = None;
        self.recovery_codes.clear();
        self.updated_at = Some(chrono::Utc::now().timestamp());
    }

    /// 设置恢复码
    pub fn set_recovery_codes(&mut self, codes: Vec<RecoveryCode>) {
        self.recovery_codes = codes;
        self.updated_at = Some(chrono::Utc::now().timestamp());
    }

    /// 获取未使用的恢复码数量
    pub fn available_recovery_codes_count(&self) -> usize {
        self.recovery_codes.iter().filter(|c| !c.used).count()
    }

    /// 是否设置了密码
    pub fn has_password(&self) -> bool {
        self.password_hash.is_some()
    }

    /// 是否启用了 TOTP
    pub fn has_totp(&self) -> bool {
        self.totp_secret.is_some()
    }
}

/// 令牌对
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenPair {
    /// Access Token
    pub access_token: String,
    /// Refresh Token
    pub refresh_token: String,
    /// Access Token 过期时间（Unix 时间戳）
    pub access_expires_at: i64,
    /// Refresh Token 过期时间（Unix 时间戳）
    pub refresh_expires_at: i64,
}

/// JWT Claims
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenClaims {
    /// Subject (固定为 "web_auth")
    pub sub: String,
    /// Issued At (签发时间)
    pub iat: i64,
    /// Expiration (过期时间)
    pub exp: i64,
    /// JWT ID (唯一标识)
    pub jti: String,
}

/// 存储的 Refresh Token 信息
#[derive(Debug, Clone)]
pub struct StoredRefreshToken {
    /// Token 哈希
    pub token_hash: String,
    /// 过期时间
    pub expires_at: i64,
    /// 创建时间
    pub created_at: i64,
}

/// 登录尝试记录
#[derive(Debug, Clone)]
pub struct LoginAttempt {
    /// 失败次数
    pub count: u32,
    /// 首次尝试时间
    pub first_attempt: i64,
    /// 锁定截止时间
    pub locked_until: Option<i64>,
}

impl LoginAttempt {
    /// 创建新的登录尝试记录
    pub fn new() -> Self {
        Self {
            count: 1,
            first_attempt: chrono::Utc::now().timestamp(),
            locked_until: None,
        }
    }
}

impl Default for LoginAttempt {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_auth_mode_requirements() {
        assert!(!AuthMode::None.requires_password());
        assert!(!AuthMode::None.requires_totp());
        assert!(!AuthMode::None.is_enabled());

        assert!(AuthMode::Password.requires_password());
        assert!(!AuthMode::Password.requires_totp());
        assert!(AuthMode::Password.is_enabled());

        assert!(!AuthMode::Totp.requires_password());
        assert!(AuthMode::Totp.requires_totp());
        assert!(AuthMode::Totp.is_enabled());

        assert!(AuthMode::PasswordTotp.requires_password());
        assert!(AuthMode::PasswordTotp.requires_totp());
        assert!(AuthMode::PasswordTotp.is_enabled());
    }

    #[test]
    fn test_auth_credentials() {
        let mut creds = AuthCredentials::new();
        assert!(!creds.has_password());
        assert!(!creds.has_totp());

        creds.set_password_hash("hash123".to_string());
        assert!(creds.has_password());
        assert!(creds.updated_at.is_some());

        creds.set_totp_secret("secret123".to_string());
        assert!(creds.has_totp());

        creds.clear_password();
        assert!(!creds.has_password());
    }

    #[test]
    fn test_recovery_code() {
        let mut code = RecoveryCode::new("hash".to_string());
        assert!(!code.used);
        assert!(code.used_at.is_none());

        code.mark_used();
        assert!(code.used);
        assert!(code.used_at.is_some());
    }
}

//! DTO:优先复用后端导出的类型(它们同时 derive Serialize/Deserialize,
//! 线缆契约天然一致);后端只 derive Serialize 的响应结构在此定义桌面侧镜像。

pub use purepan_backend::auth::types::{AccountSummary, UserAuth};
pub use purepan_backend::web_auth::types::AuthMode;
pub use purepan_backend::{QRCode, QRCodeStatus};

use serde::{Deserialize, Serialize};

// ───────── accounts(后端 handler 响应仅 Serialize) ─────────

#[derive(Debug, Clone, Deserialize)]
pub struct ListAccountsResponse {
    pub accounts: Vec<AccountSummary>,
    #[serde(default)]
    pub active_uid: Option<u64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SwitchAccountResponse {
    pub active_uid: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DeleteAccountResponse {
    pub deleted_uid: u64,
    pub new_active_uid: Option<u64>,
}

// ───────── web-auth(裸 JSON 响应) ─────────

#[derive(Debug, Clone, Serialize)]
pub struct WebLoginRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub password: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub totp_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recovery_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pending_token: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct WebLoginResponse {
    /// success / need_totp / error
    pub status: String,
    pub access_token: Option<String>,
    pub refresh_token: Option<String>,
    pub access_expires_at: Option<i64>,
    pub refresh_expires_at: Option<i64>,
    pub pending_token: Option<String>,
    pub error: Option<String>,
    pub lockout_remaining: Option<i64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RefreshResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub access_expires_at: i64,
    pub refresh_expires_at: i64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AuthStatusResponse {
    pub enabled: bool,
    pub mode: AuthMode,
    pub authenticated: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AuthConfigResponse {
    pub enabled: bool,
    pub mode: AuthMode,
    pub password_set: bool,
    pub totp_enabled: bool,
    pub recovery_codes_count: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TotpSetupResponse {
    pub secret: String,
    pub qr_code: String,
    pub issuer: String,
    pub account: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RegenerateCodesResponse {
    pub codes: Vec<String>,
}
//! DTO:优先复用后端导出的类型(它们同时 derive Serialize/Deserialize,
//! 线缆契约天然一致);后端只 derive Serialize 的响应结构在此定义桌面侧镜像。

pub use purepan_backend::auth::types::{AccountSummary, UserAuth};
pub use purepan_backend::{QRCode, QRCodeStatus};

use serde::Deserialize;

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

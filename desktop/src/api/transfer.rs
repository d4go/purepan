//! 转存(分享保存)API(进程内直调)。
//! 业务错误码(1001 需密码/1002 密码错)经 ApiError::Business.code 透出。

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use purepan_backend::server::inproc;

use super::{ApiClient, ApiError};

pub use purepan_backend::transfer::{SharedFileInfo, TransferStatus};

#[derive(Debug, Clone, Serialize, Default)]
pub struct CreateTransferRequest {
    pub share_url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub password: Option<String>,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub save_path: String,
    pub save_fs_id: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auto_download: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub local_download_path: Option<String>,
    pub is_share_direct_download: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selected_fs_ids: Option<Vec<u64>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selected_files: Option<Vec<SharedFileInfo>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uid: Option<u64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateTransferResponse {
    pub task_id: Option<String>,
    pub status: Option<TransferStatus>,
    pub need_password: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PreviewShareResponse {
    pub files: Vec<SharedFileInfo>,
    pub share_info: Option<Value>,
}

pub fn transfer_status_text(status: &str) -> &'static str {
    match status {
        "queued" => "排队中",
        "checking_share" => "检查分享...",
        "transferring" => "转存中",
        "transfer_failed" => "转存失败",
        "transferred" => "已转存",
        "downloading" => "下载中",
        "download_failed" => "下载失败",
        "cleaning" => "清理临时文件中",
        "completed" => "已完成",
        _ => "未知",
    }
}

impl ApiClient {
    pub async fn create_transfer(&self, req: &CreateTransferRequest) -> Result<CreateTransferResponse, ApiError> {
        let body = json!(req);
        self.request(move |s| inproc::transfer::create(s, body)).await
    }

    pub async fn preview_share_files(&self, share_url: &str, password: Option<&str>, page: u32, num: u32) -> Result<PreviewShareResponse, ApiError> {
        let mut body = json!({ "share_url": share_url, "page": page, "num": num });
        if let Some(p) = password { body["password"] = json!(p); }
        self.request(move |s| inproc::transfer::preview(s, body)).await
    }

    pub async fn delete_transfer(&self, id: &str) -> Result<String, ApiError> {
        let id = id.to_string();
        self.request(move |s| inproc::transfer::delete(s, id)).await
    }

    pub async fn cancel_transfer(&self, id: &str) -> Result<String, ApiError> {
        let id = id.to_string();
        self.request(move |s| inproc::transfer::cancel(s, id)).await
    }
}
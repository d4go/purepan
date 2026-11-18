//! 分享管理 API(移植 frontend/src/api/share.ts)。

use reqwest::Method;
use serde::{Deserialize, Serialize};
use serde_json::json;

use super::{ApiError, ApiClient};

#[derive(Debug, Clone, Serialize)]
pub struct CreateShareRequest {
    pub paths: Vec<String>,
    pub period: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pwd: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ShareResult {
    pub link: String,
    pub pwd: String,
    pub shareid: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CancelShareResult {
    pub success: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ShareRecordData {
    #[serde(rename = "shareId")]
    pub share_id: u64,
    #[serde(rename = "fsIds")]
    pub fs_ids: Vec<i64>,
    pub shortlink: String,
    pub status: i32,
    pub public: i32,
    #[serde(rename = "typicalCategory")]
    pub typical_category: i32,
    #[serde(rename = "typicalPath")]
    pub typical_path: String,
    #[serde(rename = "expiredType")]
    pub expired_type: i32,
    #[serde(rename = "expiredTime")]
    pub expired_time: i64,
    #[serde(rename = "viewCount")]
    pub view_count: i32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ShareListData {
    pub list: Vec<ShareRecordData>,
    pub total: u32,
    pub page: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ShareDetailData {
    pub pwd: String,
    pub shorturl: String,
}

impl ApiClient {
    /// POST /shares(period: 0=永久,1/7/30 天)
    pub async fn create_share(&self, paths: &[String], period: i32, pwd: Option<&str>) -> Result<ShareResult, ApiError> {
        let req = CreateShareRequest { paths: paths.to_vec(), period, pwd: pwd.map(String::from) };
        self.request(Method::POST, "/shares", Some(json!(req))).await
    }

    /// POST /shares/cancel
    pub async fn cancel_share(&self, share_ids: &[u64]) -> Result<CancelShareResult, ApiError> {
        self.request(Method::POST, "/shares/cancel", Some(json!({ "share_ids": share_ids }))).await
    }

    /// GET /shares?page=
    pub async fn list_shares(&self, page: u32) -> Result<ShareListData, ApiError> {
        self.request(Method::GET, &format!("/shares?page={page}"), None).await
    }

    /// GET /shares/:id
    pub async fn get_share_detail(&self, share_id: u64) -> Result<ShareDetailData, ApiError> {
        self.request(Method::GET, &format!("/shares/{share_id}"), None).await
    }
}
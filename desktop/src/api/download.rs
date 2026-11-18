//! 下载任务 API(移植 frontend/src/api/download.ts)。

use reqwest::Method;
use serde::{Deserialize, Serialize};
use serde_json::json;

use super::{ApiError, ApiClient};

pub use purepan_backend::downloader::{DownloadTask, TaskStatus};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum DownloadConflictStrategy {
    #[default]
    Overwrite,
    Skip,
    AutoRename,
}

#[derive(Debug, Clone, Serialize)]
pub struct BatchDownloadItem {
    pub fs_id: u64,
    pub path: String,
    pub name: String,
    pub is_dir: bool,
    pub size: Option<u64>,
    pub original_name: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BatchDownloadResponse {
    pub task_ids: Vec<String>,
    pub folder_task_ids: Vec<String>,
    pub failed: Vec<BatchDownloadError>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BatchDownloadError {
    pub path: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct BatchOperationRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task_ids: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub all: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delete_files: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uid: Option<u64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BatchOperationResponse {
    pub total: usize,
    pub success_count: usize,
    pub failed_count: usize,
}

impl ApiClient {
    /// POST /downloads(单文件)→ 任务 ID
    pub async fn create_download(
        &self,
        fs_id: u64,
        remote_path: &str,
        filename: &str,
        total_size: u64,
    ) -> Result<String, ApiError> {
        self.request(
            Method::POST,
            "/downloads",
            Some(json!({
                "fs_id": fs_id,
                "remote_path": remote_path,
                "filename": filename,
                "total_size": total_size,
            })),
        )
        .await
    }

    /// POST /downloads/batch(文件+文件夹混合)
    pub async fn create_downloads_batch(
        &self,
        items: &[BatchDownloadItem],
        target_dir: &str,
        conflict_strategy: Option<DownloadConflictStrategy>,
    ) -> Result<BatchDownloadResponse, ApiError> {
        let mut body = json!({ "items": items, "target_dir": target_dir });
        if let Some(c) = conflict_strategy {
            body["conflict_strategy"] = json!(c);
        }
        self.request(Method::POST, "/downloads/batch", Some(body)).await
    }

    /// GET /downloads
    pub async fn list_downloads(&self) -> Result<Vec<DownloadTask>, ApiError> {
        self.request(Method::GET, "/downloads", None).await
    }

    /// GET /downloads/all(统一混合接口)
    pub async fn list_downloads_mixed(&self) -> Result<Vec<DownloadTask>, ApiError> {
        self.request(Method::GET, "/downloads/all", None).await
    }

    /// GET /downloads/active(降级轮询用)
    pub async fn list_active_downloads(&self) -> Result<Vec<DownloadTask>, ApiError> {
        self.request(Method::GET, "/downloads/active", None).await
    }

    /// GET /downloads/:id
    pub async fn get_download(&self, id: &str) -> Result<DownloadTask, ApiError> {
        self.request(Method::GET, &format!("/downloads/{id}"), None).await
    }

    pub async fn pause_download(&self, id: &str) -> Result<String, ApiError> {
        self.request(Method::POST, &format!("/downloads/{id}/pause"), Some(json!({}))).await
    }

    pub async fn resume_download(&self, id: &str) -> Result<String, ApiError> {
        self.request(Method::POST, &format!("/downloads/{id}/resume"), Some(json!({}))).await
    }

    /// DELETE /downloads/:id?delete_files=
    pub async fn delete_download(&self, id: &str, delete_files: bool) -> Result<String, ApiError> {
        self.request(
            Method::DELETE,
            &format!("/downloads/{id}?delete_files={delete_files}"),
            None,
        )
        .await
    }

    pub async fn clear_completed_downloads(&self) -> Result<usize, ApiError> {
        self.request(Method::DELETE, "/downloads/clear/completed", None).await
    }

    pub async fn clear_failed_downloads(&self) -> Result<usize, ApiError> {
        self.request(Method::DELETE, "/downloads/clear/failed", None).await
    }

    pub async fn batch_pause_downloads(&self, req: BatchOperationRequest) -> Result<BatchOperationResponse, ApiError> {
        self.request(Method::POST, "/downloads/batch/pause", Some(json!(req))).await
    }

    pub async fn batch_resume_downloads(&self, req: BatchOperationRequest) -> Result<BatchOperationResponse, ApiError> {
        self.request(Method::POST, "/downloads/batch/resume", Some(json!(req))).await
    }

    pub async fn batch_delete_downloads(&self, req: BatchOperationRequest) -> Result<BatchOperationResponse, ApiError> {
        self.request(Method::POST, "/downloads/batch/delete", Some(json!(req))).await
    }
}
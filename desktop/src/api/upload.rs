//! 上传任务 API(移植 frontend/src/api/upload.ts)。

use reqwest::Method;
use serde::{Deserialize, Serialize};
use serde_json::json;

use super::download::{BatchOperationRequest, BatchOperationResponse};
use super::{ApiError, ApiClient};

pub use purepan_backend::uploader::{UploadTask, UploadTaskStatus};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum UploadConflictStrategy {
    #[default]
    SmartDedup,
    AutoRename,
    Overwrite,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ScanStartResponse {
    pub scan_task_id: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ScanStatusResponse {
    pub scan_task_id: String,
    pub status: String,
    pub scanned_files: usize,
    pub created_tasks: usize,
    pub skipped_duplicates: usize,
    pub total_size: u64,
    pub owner_uid: Option<u64>,
}

impl ApiClient {
    /// POST /uploads(单文件)→ 任务 ID
    pub async fn create_upload(
        &self,
        local_path: &str,
        remote_path: &str,
        encrypt: bool,
        conflict_strategy: Option<UploadConflictStrategy>,
    ) -> Result<String, ApiError> {
        let mut body = json!({ "local_path": local_path, "remote_path": remote_path, "encrypt": encrypt });
        if let Some(c) = conflict_strategy {
            body["conflict_strategy"] = json!(c);
        }
        self.request(Method::POST, "/uploads", Some(body)).await
    }

    /// POST /uploads/batch(files: [(local, remote)])
    pub async fn create_batch_upload(
        &self,
        files: &[(String, String)],
        encrypt: bool,
        conflict_strategy: Option<UploadConflictStrategy>,
    ) -> Result<Vec<String>, ApiError> {
        let mut body = json!({ "files": files, "encrypt": encrypt });
        if let Some(c) = conflict_strategy {
            body["conflict_strategy"] = json!(c);
        }
        self.request(Method::POST, "/uploads/batch", Some(body)).await
    }

    /// POST /uploads/folder → 扫描任务 ID
    pub async fn create_folder_upload(
        &self,
        local_folder: &str,
        remote_folder: &str,
        encrypt: bool,
        conflict_strategy: Option<UploadConflictStrategy>,
    ) -> Result<ScanStartResponse, ApiError> {
        let mut body = json!({ "local_folder": local_folder, "remote_folder": remote_folder, "encrypt": encrypt });
        if let Some(c) = conflict_strategy {
            body["conflict_strategy"] = json!(c);
        }
        self.request(Method::POST, "/uploads/folder", Some(body)).await
    }

    /// GET /uploads
    pub async fn list_uploads(&self) -> Result<Vec<UploadTask>, ApiError> {
        self.request(Method::GET, "/uploads", None).await
    }

    pub async fn get_upload(&self, id: &str) -> Result<UploadTask, ApiError> {
        self.request(Method::GET, &format!("/uploads/{id}"), None).await
    }

    pub async fn pause_upload(&self, id: &str) -> Result<String, ApiError> {
        self.request(Method::POST, &format!("/uploads/{id}/pause"), Some(json!({}))).await
    }

    pub async fn resume_upload(&self, id: &str) -> Result<String, ApiError> {
        self.request(Method::POST, &format!("/uploads/{id}/resume"), Some(json!({}))).await
    }

    pub async fn delete_upload(&self, id: &str) -> Result<String, ApiError> {
        self.request(Method::DELETE, &format!("/uploads/{id}"), None).await
    }

    pub async fn clear_completed_uploads(&self) -> Result<usize, ApiError> {
        self.request(Method::DELETE, "/uploads/clear/completed", None).await
    }

    pub async fn clear_failed_uploads(&self) -> Result<usize, ApiError> {
        self.request(Method::DELETE, "/uploads/clear/failed", None).await
    }

    /// GET /uploads/scan/:id
    pub async fn get_scan(&self, id: &str) -> Result<ScanStatusResponse, ApiError> {
        self.request(Method::GET, &format!("/uploads/scan/{id}"), None).await
    }

    /// POST /uploads/scan/:id/cancel
    pub async fn cancel_scan(&self, id: &str) -> Result<String, ApiError> {
        self.request(Method::POST, &format!("/uploads/scan/{id}/cancel"), Some(json!({}))).await
    }

    pub async fn batch_pause_uploads(&self, req: BatchOperationRequest) -> Result<BatchOperationResponse, ApiError> {
        self.request(Method::POST, "/uploads/batch/pause", Some(json!(req))).await
    }

    pub async fn batch_resume_uploads(&self, req: BatchOperationRequest) -> Result<BatchOperationResponse, ApiError> {
        self.request(Method::POST, "/uploads/batch/resume", Some(json!(req))).await
    }

    pub async fn batch_delete_uploads(&self, req: BatchOperationRequest) -> Result<BatchOperationResponse, ApiError> {
        self.request(Method::POST, "/uploads/batch/delete", Some(json!(req))).await
    }
}
//! 下载任务 API(进程内直调)。

use serde::{Deserialize, Serialize};
use serde_json::json;

use purepan_backend::server::inproc;

use super::{ApiClient, ApiError};

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
        self.request(move |s| inproc::download::create_batch(s, body))
            .await
    }

    pub async fn pause_download(&self, id: &str) -> Result<String, ApiError> {
        let id = id.to_string();
        self.request(move |s| inproc::download::pause(s, id)).await
    }

    pub async fn resume_download(&self, id: &str) -> Result<String, ApiError> {
        let id = id.to_string();
        self.request(move |s| inproc::download::resume(s, id)).await
    }

    pub async fn delete_download(&self, id: &str, delete_files: bool) -> Result<String, ApiError> {
        let id = id.to_string();
        self.request(move |s| inproc::download::delete(s, id, delete_files))
            .await
    }

    pub async fn clear_completed_downloads(&self) -> Result<usize, ApiError> {
        self.request(inproc::download::clear_completed).await
    }

    pub async fn clear_failed_downloads(&self) -> Result<usize, ApiError> {
        self.request(inproc::download::clear_failed).await
    }

    pub async fn pause_folder_download(&self, id: &str) -> Result<String, ApiError> {
        let id = id.to_string();
        self.request(move |s| inproc::download::pause_folder(s, id))
            .await
    }

    pub async fn resume_folder_download(&self, id: &str) -> Result<String, ApiError> {
        let id = id.to_string();
        self.request(move |s| inproc::download::resume_folder(s, id))
            .await
    }

    pub async fn cancel_folder_download(
        &self,
        id: &str,
        delete_files: bool,
    ) -> Result<String, ApiError> {
        let id = id.to_string();
        self.request(move |s| inproc::download::cancel_folder(s, id, delete_files))
            .await
    }

    pub async fn batch_pause_downloads(
        &self,
        req: BatchOperationRequest,
    ) -> Result<BatchOperationResponse, ApiError> {
        let body = json!(req);
        self.request(move |s| inproc::download::batch_pause(s, body))
            .await
    }

    pub async fn batch_resume_downloads(
        &self,
        req: BatchOperationRequest,
    ) -> Result<BatchOperationResponse, ApiError> {
        let body = json!(req);
        self.request(move |s| inproc::download::batch_resume(s, body))
            .await
    }
}

//! 离线下载 API(移植 frontend/src/api/cloudDl.ts)。active-only 语义。

use reqwest::Method;
use serde::{Deserialize, Serialize};
use serde_json::json;

use super::{ApiError, ApiClient};

pub use purepan_backend::netdisk::cloud_dl::CloudDlTaskInfo;

#[derive(Debug, Clone, Serialize, Default)]
pub struct AddTaskRequest {
    pub source_url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub save_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auto_download: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub local_download_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ask_download_path: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AddTaskResponse {
    pub task_id: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TaskListResponse {
    pub tasks: Vec<CloudDlTaskInfo>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ClearTasksResponse {
    pub total: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OperationResponse {
    pub success: bool,
    pub message: Option<String>,
}

impl ApiClient {
    /// POST /cloud-dl/tasks
    pub async fn cloud_dl_add(&self, req: &AddTaskRequest) -> Result<AddTaskResponse, ApiError> {
        self.request(Method::POST, "/cloud-dl/tasks", Some(json!(req))).await
    }

    /// GET /cloud-dl/tasks
    pub async fn cloud_dl_list(&self) -> Result<TaskListResponse, ApiError> {
        self.request(Method::GET, "/cloud-dl/tasks", None).await
    }

    /// GET /cloud-dl/tasks/:id
    pub async fn cloud_dl_query(&self, task_id: u64) -> Result<CloudDlTaskInfo, ApiError> {
        self.request(Method::GET, &format!("/cloud-dl/tasks/{task_id}"), None).await
    }

    /// POST /cloud-dl/tasks/:id/cancel
    pub async fn cloud_dl_cancel(&self, task_id: u64) -> Result<OperationResponse, ApiError> {
        self.request(Method::POST, &format!("/cloud-dl/tasks/{task_id}/cancel"), Some(json!({}))).await
    }

    /// DELETE /cloud-dl/tasks/:id
    pub async fn cloud_dl_delete(&self, task_id: u64) -> Result<OperationResponse, ApiError> {
        self.request(Method::DELETE, &format!("/cloud-dl/tasks/{task_id}"), None).await
    }

    /// DELETE /cloud-dl/tasks/clear
    pub async fn cloud_dl_clear(&self) -> Result<ClearTasksResponse, ApiError> {
        self.request(Method::DELETE, "/cloud-dl/tasks/clear", None).await
    }

    /// POST /cloud-dl/tasks/refresh
    pub async fn cloud_dl_refresh(&self) -> Result<TaskListResponse, ApiError> {
        self.request(Method::POST, "/cloud-dl/tasks/refresh", Some(json!({}))).await
    }
}
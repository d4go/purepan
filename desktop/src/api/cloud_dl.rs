//! 离线下载 API(进程内直调)。active-only 语义。

use serde::{Deserialize, Serialize};
use serde_json::json;

use purepan_backend::server::inproc;

use super::{ApiClient, ApiError};

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
    pub async fn cloud_dl_add(&self, req: &AddTaskRequest) -> Result<AddTaskResponse, ApiError> {
        let body = json!(req);
        self.request(move |s| inproc::cloud_dl::add(s, body)).await
    }

    pub async fn cloud_dl_list(&self) -> Result<TaskListResponse, ApiError> {
        self.request(inproc::cloud_dl::list).await
    }

    pub async fn cloud_dl_cancel(&self, task_id: u64) -> Result<OperationResponse, ApiError> {
        let task_id = task_id as i64;
        self.request(move |s| inproc::cloud_dl::cancel(s, task_id))
            .await
    }

    pub async fn cloud_dl_delete(&self, task_id: u64) -> Result<OperationResponse, ApiError> {
        let task_id = task_id as i64;
        self.request(move |s| inproc::cloud_dl::delete(s, task_id))
            .await
    }

    pub async fn cloud_dl_clear(&self) -> Result<ClearTasksResponse, ApiError> {
        self.request(inproc::cloud_dl::clear).await
    }
}

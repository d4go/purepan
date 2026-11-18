//! 自动备份 + 加密 API(移植 frontend/src/api/autobackup.ts)。{success,data,error} 信封。

use reqwest::Method;
use serde::Deserialize;
use serde_json::{Value, json};

use super::{ApiError, ApiClient};

pub use purepan_backend::autobackup::config::BackupConfig;
pub use purepan_backend::autobackup::task::{BackupFileTask, BackupTask};

#[derive(Debug, Clone, Deserialize)]
pub struct BackupTasksResponse {
    pub tasks: Vec<BackupTask>,
    pub total: usize,
    pub page: usize,
    pub page_size: usize,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FileTasksResponse {
    pub file_tasks: Vec<BackupFileTask>,
    pub total: usize,
    pub page: usize,
    pub page_size: usize,
}

#[derive(Debug, Clone, Deserialize)]
pub struct EncryptionStatusResponse {
    pub enabled: bool,
    pub has_key: bool,
    pub algorithm: String,
    pub key_created_at: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GenerateKeyResponse {
    pub key: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TriggerResponse {
    pub task_id: String,
}

impl ApiClient {
    pub async fn list_backup_configs(&self) -> Result<Vec<BackupConfig>, ApiError> {
        self.request_success(Method::GET, "/autobackup/configs", None).await
    }

    pub async fn get_backup_config(&self, id: &str) -> Result<BackupConfig, ApiError> {
        self.request_success(Method::GET, &format!("/autobackup/configs/{id}"), None).await
    }

    pub async fn create_backup_config(&self, body: Value) -> Result<BackupConfig, ApiError> {
        self.request_success(Method::POST, "/autobackup/configs", Some(body)).await
    }

    pub async fn update_backup_config(&self, id: &str, body: Value) -> Result<BackupConfig, ApiError> {
        self.request_success(Method::PUT, &format!("/autobackup/configs/{id}"), Some(body)).await
    }

    pub async fn delete_backup_config(&self, id: &str) -> Result<Value, ApiError> {
        self.request_success(Method::DELETE, &format!("/autobackup/configs/{id}"), None).await
    }

    pub async fn enable_backup_config(&self, id: &str) -> Result<BackupConfig, ApiError> {
        self.request_success(Method::POST, &format!("/autobackup/configs/{id}/enable"), Some(json!({}))).await
    }

    pub async fn disable_backup_config(&self, id: &str) -> Result<BackupConfig, ApiError> {
        self.request_success(Method::POST, &format!("/autobackup/configs/{id}/disable"), Some(json!({}))).await
    }

    pub async fn trigger_backup(&self, config_id: &str) -> Result<TriggerResponse, ApiError> {
        self.request_success(Method::POST, &format!("/autobackup/configs/{config_id}/trigger"), Some(json!({}))).await
    }

    pub async fn get_backup_task(&self, task_id: &str) -> Result<BackupTask, ApiError> {
        self.request_success(Method::GET, &format!("/autobackup/tasks/{task_id}"), None).await
    }

    pub async fn list_backup_tasks(&self, config_id: &str, page: usize) -> Result<BackupTasksResponse, ApiError> {
        self.request_success(Method::GET, &format!("/autobackup/configs/{config_id}/tasks?page={page}&page_size=20"), None).await
    }

    pub async fn cancel_backup_task(&self, task_id: &str) -> Result<Value, ApiError> {
        self.request_success(Method::POST, &format!("/autobackup/tasks/{task_id}/cancel"), Some(json!({}))).await
    }

    pub async fn pause_backup_task(&self, task_id: &str) -> Result<Value, ApiError> {
        self.request_success(Method::POST, &format!("/autobackup/tasks/{task_id}/pause"), Some(json!({}))).await
    }

    pub async fn resume_backup_task(&self, task_id: &str) -> Result<Value, ApiError> {
        self.request_success(Method::POST, &format!("/autobackup/tasks/{task_id}/resume"), Some(json!({}))).await
    }

    pub async fn list_file_tasks(&self, task_id: &str, page: usize) -> Result<FileTasksResponse, ApiError> {
        self.request_success(Method::GET, &format!("/autobackup/tasks/{task_id}/files?page={page}&page_size=20"), None).await
    }

    pub async fn retry_file_task(&self, task_id: &str, file_task_id: &str) -> Result<Value, ApiError> {
        self.request_success(Method::POST, &format!("/autobackup/tasks/{task_id}/files/{file_task_id}/retry"), Some(json!({}))).await
    }

    // ── 加密 ──
    pub async fn encryption_status(&self) -> Result<EncryptionStatusResponse, ApiError> {
        self.request_success(Method::GET, "/encryption/status", None).await
    }

    pub async fn generate_encryption_key(&self, algorithm: Option<&str>) -> Result<String, ApiError> {
        let body = json!({ "algorithm": algorithm });
        let resp: GenerateKeyResponse = self.request_success(Method::POST, "/encryption/key/generate", Some(body)).await?;
        Ok(resp.key)
    }

    pub async fn import_encryption_key(&self, key: &str, algorithm: Option<&str>) -> Result<Value, ApiError> {
        self.request_success(Method::POST, "/encryption/key/import", Some(json!({ "key": key, "algorithm": algorithm }))).await
    }

    pub async fn export_encryption_key(&self) -> Result<String, ApiError> {
        let resp: GenerateKeyResponse = self.request_success(Method::GET, "/encryption/key/export", None).await?;
        Ok(resp.key)
    }

    pub async fn delete_encryption_key(&self, force: bool) -> Result<Value, ApiError> {
        let path = if force { "/encryption/key/force" } else { "/encryption/key" };
        self.request_success(Method::DELETE, path, None).await
    }
}
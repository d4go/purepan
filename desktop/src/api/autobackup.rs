//! 自动备份 + 加密 API(进程内直调)。

use serde::Deserialize;
use serde_json::Value;

use purepan_backend::server::inproc;

use super::{ApiClient, ApiError};

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
        self.request(inproc::autobackup::list_configs).await
    }

    pub async fn create_backup_config(&self, body: Value) -> Result<BackupConfig, ApiError> {
        self.request(move |s| inproc::autobackup::create_config(s, body))
            .await
    }

    pub async fn delete_backup_config(&self, id: &str) -> Result<Value, ApiError> {
        let id = id.to_string();
        self.request(move |s| inproc::autobackup::delete_config(s, id))
            .await
    }

    pub async fn enable_backup_config(&self, id: &str) -> Result<BackupConfig, ApiError> {
        let id = id.to_string();
        self.request(move |s| inproc::autobackup::enable_config(s, id))
            .await
    }

    pub async fn disable_backup_config(&self, id: &str) -> Result<BackupConfig, ApiError> {
        let id = id.to_string();
        self.request(move |s| inproc::autobackup::disable_config(s, id))
            .await
    }

    pub async fn trigger_backup(&self, config_id: &str) -> Result<TriggerResponse, ApiError> {
        let config_id = config_id.to_string();
        self.request(move |s| inproc::autobackup::trigger(s, config_id))
            .await
    }

    pub async fn list_backup_tasks(&self, config_id: &str, page: usize) -> Result<BackupTasksResponse, ApiError> {
        let config_id = config_id.to_string();
        self.request(move |s| inproc::autobackup::list_tasks(s, config_id, page))
            .await
    }

    pub async fn list_file_tasks(&self, task_id: &str, page: usize) -> Result<FileTasksResponse, ApiError> {
        let task_id = task_id.to_string();
        self.request(move |s| inproc::autobackup::list_file_tasks(s, task_id, page))
            .await
    }

    pub async fn retry_file_task(&self, task_id: &str, file_task_id: &str) -> Result<Value, ApiError> {
        let (task_id, file_task_id) = (task_id.to_string(), file_task_id.to_string());
        self.request(move |s| inproc::autobackup::retry_file_task(s, task_id, file_task_id))
            .await
    }

    // ── 加密 ──
    pub async fn encryption_status(&self) -> Result<EncryptionStatusResponse, ApiError> {
        self.request(inproc::autobackup::encryption_status).await
    }

    pub async fn generate_encryption_key(&self, algorithm: Option<&str>) -> Result<String, ApiError> {
        let algorithm = algorithm.map(String::from);
        let resp: GenerateKeyResponse = self
            .request(move |s| inproc::autobackup::generate_key(s, algorithm))
            .await?;
        Ok(resp.key)
    }

    pub async fn import_encryption_key(&self, key: &str, algorithm: Option<&str>) -> Result<Value, ApiError> {
        let (key, algorithm) = (key.to_string(), algorithm.map(String::from));
        self.request(move |s| inproc::autobackup::import_key(s, key, algorithm))
            .await
    }

    pub async fn export_encryption_key(&self) -> Result<String, ApiError> {
        let resp: GenerateKeyResponse = self.request(inproc::autobackup::export_key).await?;
        Ok(resp.key)
    }

    pub async fn delete_encryption_key(&self, force: bool) -> Result<Value, ApiError> {
        self.request(move |s| inproc::autobackup::delete_key(s, force))
            .await
    }
}

//! 上传任务 API(进程内直调)。

use std::path::{Path, PathBuf};

use serde::Deserialize;
use serde_json::json;

use purepan_backend::server::inproc;

use super::download::{BatchOperationRequest, BatchOperationResponse};
use super::file::join_path;
use super::{ApiClient, ApiError};

#[derive(Debug, Clone, Deserialize)]
pub struct ScanStartResponse {
    pub scan_task_id: String,
}

impl ApiClient {
    /// 整批任务固定到提交时的账号，避免上传期间切换账号导致任务串号。
    pub async fn queue_local_paths(
        &self,
        paths: Vec<PathBuf>,
        remote_dir: &str,
        uid: u64,
    ) -> Result<usize, ApiError> {
        let state = self.state().await?;
        let active_uid = state.active_uid.read().await.map(|uid| uid.raw());
        if active_uid != Some(uid) {
            return Err(ApiError::Business {
                code: 409,
                message: "当前账号已切换，请重新选择或拖入文件".into(),
            });
        }

        let mut queued = 0;
        let mut first_error = None;
        for path in paths {
            match self.queue_local_path(&path, remote_dir, uid).await {
                Ok(()) => queued += 1,
                Err(error) => {
                    first_error.get_or_insert(error);
                }
            }
        }
        if queued == 0
            && let Some(error) = first_error
        {
            return Err(error);
        }
        Ok(queued)
    }

    async fn queue_local_path(
        &self,
        path: &Path,
        remote_dir: &str,
        uid: u64,
    ) -> Result<(), ApiError> {
        let local_path = path.to_str().ok_or_else(|| ApiError::Business {
            code: 400,
            message: "文件路径不是有效的 UTF-8".into(),
        })?;
        let metadata = tokio::fs::metadata(path)
            .await
            .map_err(|error| ApiError::Business {
                code: 400,
                message: format!("无法读取本地路径 {}：{error}", path.display()),
            })?;
        if metadata.is_dir() {
            self.create_folder_upload(local_path, remote_dir, uid)
                .await?;
        } else if metadata.is_file() {
            let name = path
                .file_name()
                .and_then(|name| name.to_str())
                .ok_or_else(|| ApiError::Business {
                    code: 400,
                    message: "无法读取文件名".into(),
                })?;
            self.create_upload(local_path, &join_path(remote_dir, name), uid)
                .await?;
        } else {
            return Err(ApiError::Business {
                code: 400,
                message: "请选择普通文件或文件夹".into(),
            });
        }
        Ok(())
    }

    async fn create_upload(
        &self,
        local_path: &str,
        remote_path: &str,
        uid: u64,
    ) -> Result<String, ApiError> {
        let body = json!({
            "local_path": local_path,
            "remote_path": remote_path,
            "uid": uid,
        });
        self.request(move |s| inproc::upload::create(s, body)).await
    }

    async fn create_folder_upload(
        &self,
        local_folder: &str,
        remote_folder: &str,
        uid: u64,
    ) -> Result<ScanStartResponse, ApiError> {
        let body = json!({
            "local_folder": local_folder,
            "remote_folder": remote_folder,
            "uid": uid,
        });
        self.request(move |s| inproc::upload::create_folder(s, body))
            .await
    }

    pub async fn pause_upload(&self, id: &str) -> Result<String, ApiError> {
        let id = id.to_string();
        self.request(move |s| inproc::upload::pause(s, id)).await
    }

    pub async fn resume_upload(&self, id: &str) -> Result<String, ApiError> {
        let id = id.to_string();
        self.request(move |s| inproc::upload::resume(s, id)).await
    }

    pub async fn delete_upload(&self, id: &str) -> Result<String, ApiError> {
        let id = id.to_string();
        self.request(move |s| inproc::upload::delete(s, id)).await
    }

    pub async fn clear_completed_uploads(&self) -> Result<usize, ApiError> {
        self.request(inproc::upload::clear_completed).await
    }

    pub async fn clear_failed_uploads(&self) -> Result<usize, ApiError> {
        self.request(inproc::upload::clear_failed).await
    }

    pub async fn batch_pause_uploads(
        &self,
        req: BatchOperationRequest,
    ) -> Result<BatchOperationResponse, ApiError> {
        let body = json!(req);
        self.request(move |s| inproc::upload::batch_pause(s, body))
            .await
    }

    pub async fn batch_resume_uploads(
        &self,
        req: BatchOperationRequest,
    ) -> Result<BatchOperationResponse, ApiError> {
        let body = json!(req);
        self.request(move |s| inproc::upload::batch_resume(s, body))
            .await
    }
}

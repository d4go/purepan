//! 自动备份 / 加密进程内门面。

use axum::extract::{Path, Query, State};
use axum::Json;
use serde_json::{json, Value};

use super::{parse_body, success_envelope, InprocError};
use crate::server::handlers::autobackup::{self as ab, BackupTasksQuery, FileTasksQuery};
use crate::server::AppState;

fn unwrap_ab<T: serde::Serialize>(resp: ab::ApiResponse<T>) -> Result<Value, InprocError> {
    success_envelope(resp.success, resp.error, resp.data)
}

fn api_err(e: crate::server::error::ApiError) -> InprocError {
    InprocError::from_api_error(&e)
}

pub async fn list_configs(state: AppState) -> Result<Value, InprocError> {
    let Json(resp) = ab::list_backup_configs(State(state)).await.map_err(api_err)?;
    unwrap_ab(resp)
}

pub async fn create_config(state: AppState, body: Value) -> Result<Value, InprocError> {
    let req: crate::autobackup::config::CreateBackupConfigRequest = parse_body(body)?;
    let Json(resp) = ab::create_backup_config(State(state), Json(req))
        .await
        .map_err(api_err)?;
    unwrap_ab(resp)
}

pub async fn delete_config(state: AppState, id: String) -> Result<Value, InprocError> {
    let Json(resp) = ab::delete_backup_config(State(state), Path(id))
        .await
        .map_err(api_err)?;
    unwrap_ab(resp)
}

pub async fn enable_config(state: AppState, id: String) -> Result<Value, InprocError> {
    let Json(resp) = ab::enable_backup_config(State(state), Path(id))
        .await
        .map_err(api_err)?;
    unwrap_ab(resp)
}

pub async fn disable_config(state: AppState, id: String) -> Result<Value, InprocError> {
    let Json(resp) = ab::disable_backup_config(State(state), Path(id))
        .await
        .map_err(api_err)?;
    unwrap_ab(resp)
}

pub async fn trigger(state: AppState, config_id: String) -> Result<Value, InprocError> {
    let Json(resp) = ab::trigger_backup(State(state), Path(config_id))
        .await
        .map_err(api_err)?;
    unwrap_ab(resp)
}

pub async fn list_tasks(
    state: AppState,
    config_id: String,
    page: usize,
) -> Result<Value, InprocError> {
    let params = BackupTasksQuery {
        page: Some(page),
        page_size: Some(20),
    };
    let Json(resp) = ab::list_backup_tasks(State(state), Path(config_id), Query(params))
        .await
        .map_err(api_err)?;
    unwrap_ab(resp)
}

pub async fn list_file_tasks(
    state: AppState,
    task_id: String,
    page: usize,
) -> Result<Value, InprocError> {
    let params = FileTasksQuery {
        page: Some(page),
        page_size: Some(20),
    };
    let Json(resp) = ab::list_file_tasks(State(state), Path(task_id), Query(params))
        .await
        .map_err(api_err)?;
    unwrap_ab(resp)
}

pub async fn retry_file_task(
    state: AppState,
    task_id: String,
    file_task_id: String,
) -> Result<Value, InprocError> {
    let Json(resp) =
        ab::retry_file_task(State(state), Path((task_id, file_task_id)))
            .await
            .map_err(api_err)?;
    unwrap_ab(resp)
}

pub async fn encryption_status(state: AppState) -> Result<Value, InprocError> {
    let Json(resp) = ab::get_encryption_status(State(state))
        .await
        .map_err(api_err)?;
    unwrap_ab(resp)
}

pub async fn generate_key(state: AppState, algorithm: Option<String>) -> Result<Value, InprocError> {
    let req: ab::GenerateKeyRequest = parse_body(json!({ "algorithm": algorithm }))?;
    let Json(resp) = ab::generate_encryption_key(State(state), Json(req))
        .await
        .map_err(api_err)?;
    unwrap_ab(resp)
}

pub async fn import_key(
    state: AppState,
    key: String,
    algorithm: Option<String>,
) -> Result<Value, InprocError> {
    let req: ab::ImportKeyRequest = parse_body(json!({ "key": key, "algorithm": algorithm }))?;
    let Json(resp) = ab::import_encryption_key(State(state), Json(req))
        .await
        .map_err(api_err)?;
    unwrap_ab(resp)
}

pub async fn export_key(state: AppState) -> Result<Value, InprocError> {
    let Json(resp) = ab::export_encryption_key(State(state))
        .await
        .map_err(api_err)?;
    unwrap_ab(resp)
}

pub async fn delete_key(state: AppState, force: bool) -> Result<Value, InprocError> {
    let Json(resp) = if force {
        ab::force_delete_encryption_key(State(state)).await
    } else {
        ab::delete_encryption_key(State(state)).await
    }
    .map_err(api_err)?;
    unwrap_ab(resp)
}

//! 下载任务进程内门面。

use axum::extract::{Path, Query, State};
use axum::Json;
use serde_json::Value;

use super::{envelope, parse_body, InprocError};
use crate::server::handlers::download::{
    self, ClearScopeQuery, DeleteDownloadQuery,
};
use crate::server::handlers::folder_download::{self, DeleteFolderQuery};
use crate::server::AppState;

pub async fn create_batch(state: AppState, body: Value) -> Result<Value, InprocError> {
    let req: download::CreateBatchDownloadRequest = parse_body(body)?;
    let Json(resp) = download::create_batch_download(State(state), Json(req))
        .await
        .map_err(InprocError::status)?;
    envelope(resp.code, resp.message, resp.data)
}

pub async fn pause(state: AppState, task_id: String) -> Result<Value, InprocError> {
    let Json(resp) = download::pause_download(State(state), Path(task_id))
        .await
        .map_err(InprocError::status)?;
    envelope(resp.code, resp.message, resp.data)
}

pub async fn resume(state: AppState, task_id: String) -> Result<Value, InprocError> {
    let Json(resp) = download::resume_download(State(state), Path(task_id))
        .await
        .map_err(InprocError::status)?;
    envelope(resp.code, resp.message, resp.data)
}

pub async fn delete(
    state: AppState,
    task_id: String,
    delete_file: bool,
) -> Result<Value, InprocError> {
    let Json(resp) = download::delete_download(
        State(state),
        Path(task_id),
        Query(DeleteDownloadQuery { delete_file }),
    )
    .await
    .map_err(InprocError::status)?;
    envelope(resp.code, resp.message, resp.data)
}

pub async fn clear_completed(state: AppState) -> Result<Value, InprocError> {
    let Json(resp) = download::clear_completed(State(state), Query(ClearScopeQuery { uid: None }))
        .await
        .map_err(InprocError::status)?;
    envelope(resp.code, resp.message, resp.data)
}

pub async fn clear_failed(state: AppState) -> Result<Value, InprocError> {
    let Json(resp) = download::clear_failed(State(state), Query(ClearScopeQuery { uid: None }))
        .await
        .map_err(InprocError::status)?;
    envelope(resp.code, resp.message, resp.data)
}

pub async fn batch_pause(state: AppState, body: Value) -> Result<Value, InprocError> {
    let req: crate::server::handlers::common::BatchOperationRequest = parse_body(body)?;
    let Json(resp) = download::batch_pause_downloads(State(state), Json(req))
        .await
        .map_err(InprocError::status)?;
    envelope(resp.code, resp.message, resp.data)
}

pub async fn batch_resume(state: AppState, body: Value) -> Result<Value, InprocError> {
    let req: crate::server::handlers::common::BatchOperationRequest = parse_body(body)?;
    let Json(resp) = download::batch_resume_downloads(State(state), Json(req))
        .await
        .map_err(InprocError::status)?;
    envelope(resp.code, resp.message, resp.data)
}

pub async fn pause_folder(state: AppState, folder_id: String) -> Result<Value, InprocError> {
    let Json(resp) = folder_download::pause_folder_download(State(state), Path(folder_id))
        .await
        .map_err(InprocError::status)?;
    envelope(resp.code, resp.message, resp.data)
}

pub async fn resume_folder(state: AppState, folder_id: String) -> Result<Value, InprocError> {
    let Json(resp) = folder_download::resume_folder_download(State(state), Path(folder_id))
        .await
        .map_err(InprocError::status)?;
    envelope(resp.code, resp.message, resp.data)
}

pub async fn cancel_folder(
    state: AppState,
    folder_id: String,
    delete_files: bool,
) -> Result<Value, InprocError> {
    let Json(resp) = folder_download::cancel_folder_download(
        State(state),
        Path(folder_id),
        Query(DeleteFolderQuery { delete_files }),
    )
    .await
    .map_err(InprocError::status)?;
    envelope(resp.code, resp.message, resp.data)
}

//! 上传任务进程内门面。

use axum::extract::{Path, Query, State};
use axum::Json;
use serde_json::Value;

use super::{envelope, parse_body, InprocError};
use crate::server::handlers::upload::{self, UploadClearScopeQuery};
use crate::server::AppState;

pub async fn create(state: AppState, body: Value) -> Result<Value, InprocError> {
    let req: upload::CreateUploadRequest = parse_body(body)?;
    let Json(resp) = upload::create_upload(State(state), Json(req))
        .await
        .map_err(|e| InprocError::from_api_error(&e))?;
    envelope(resp.code, resp.message, resp.data)
}

pub async fn create_folder(state: AppState, body: Value) -> Result<Value, InprocError> {
    let req: upload::CreateFolderUploadRequest = parse_body(body)?;
    let Json(resp) = upload::create_folder_upload(State(state), Json(req))
        .await
        .map_err(|e| InprocError::from_api_error(&e))?;
    envelope(resp.code, resp.message, resp.data)
}

pub async fn pause(state: AppState, task_id: String) -> Result<Value, InprocError> {
    let Json(resp) = upload::pause_upload(State(state), Path(task_id))
        .await
        .map_err(InprocError::status)?;
    envelope(resp.code, resp.message, resp.data)
}

pub async fn resume(state: AppState, task_id: String) -> Result<Value, InprocError> {
    let Json(resp) = upload::resume_upload(State(state), Path(task_id))
        .await
        .map_err(InprocError::status)?;
    envelope(resp.code, resp.message, resp.data)
}

pub async fn delete(state: AppState, task_id: String) -> Result<Value, InprocError> {
    let Json(resp) = upload::delete_upload(State(state), Path(task_id))
        .await
        .map_err(InprocError::status)?;
    envelope(resp.code, resp.message, resp.data)
}

pub async fn clear_completed(state: AppState) -> Result<Value, InprocError> {
    let Json(resp) =
        upload::clear_completed_uploads(State(state), Query(UploadClearScopeQuery { uid: None }))
            .await
            .map_err(InprocError::status)?;
    envelope(resp.code, resp.message, resp.data)
}

pub async fn clear_failed(state: AppState) -> Result<Value, InprocError> {
    let Json(resp) =
        upload::clear_failed_uploads(State(state), Query(UploadClearScopeQuery { uid: None }))
            .await
            .map_err(InprocError::status)?;
    envelope(resp.code, resp.message, resp.data)
}

pub async fn batch_pause(state: AppState, body: Value) -> Result<Value, InprocError> {
    let req: crate::server::handlers::common::BatchOperationRequest = parse_body(body)?;
    let Json(resp) = upload::batch_pause_uploads(State(state), Json(req))
        .await
        .map_err(InprocError::status)?;
    envelope(resp.code, resp.message, resp.data)
}

pub async fn batch_resume(state: AppState, body: Value) -> Result<Value, InprocError> {
    let req: crate::server::handlers::common::BatchOperationRequest = parse_body(body)?;
    let Json(resp) = upload::batch_resume_uploads(State(state), Json(req))
        .await
        .map_err(InprocError::status)?;
    envelope(resp.code, resp.message, resp.data)
}

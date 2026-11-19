//! 离线下载进程内门面。

use axum::extract::{Path, State};
use axum::Json;
use serde_json::Value;

use super::{envelope, parse_body, InprocError};
use crate::server::handlers::cloud_dl;
use crate::server::AppState;

pub async fn add(state: AppState, body: Value) -> Result<Value, InprocError> {
    let req: crate::netdisk::cloud_dl::AddTaskRequest = parse_body(body)?;
    let Json(resp) = cloud_dl::add_task(State(state), Json(req))
        .await
        .map_err(InprocError::status)?;
    envelope(resp.code, resp.message, resp.data)
}

pub async fn list(state: AppState) -> Result<Value, InprocError> {
    let Json(resp) = cloud_dl::list_tasks(State(state))
        .await
        .map_err(InprocError::status)?;
    envelope(resp.code, resp.message, resp.data)
}

pub async fn cancel(state: AppState, task_id: i64) -> Result<Value, InprocError> {
    let Json(resp) = cloud_dl::cancel_task(State(state), Path(task_id))
        .await
        .map_err(InprocError::status)?;
    envelope(resp.code, resp.message, resp.data)
}

pub async fn delete(state: AppState, task_id: i64) -> Result<Value, InprocError> {
    let Json(resp) = cloud_dl::delete_task(State(state), Path(task_id))
        .await
        .map_err(InprocError::status)?;
    envelope(resp.code, resp.message, resp.data)
}

pub async fn clear(state: AppState) -> Result<Value, InprocError> {
    let Json(resp) = cloud_dl::clear_tasks(State(state))
        .await
        .map_err(InprocError::status)?;
    envelope(resp.code, resp.message, resp.data)
}

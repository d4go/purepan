//! 转存（分享保存）进程内门面。

use axum::extract::{Path, State};
use axum::Json;
use serde_json::Value;

use super::{envelope, parse_body, InprocError};
use crate::server::handlers::transfer::{self, TransferApiResponse};
use crate::server::AppState;

fn unwrap_transfer<T: serde::Serialize>(
    resp: Json<TransferApiResponse<T>>,
) -> Result<Value, InprocError> {
    let resp = resp.0;
    envelope(resp.code, resp.message, resp.data)
}

pub async fn create(state: AppState, body: Value) -> Result<Value, InprocError> {
    let req: transfer::CreateTransferRequest = parse_body(body)?;
    unwrap_transfer(transfer::create_transfer(State(state), Json(req)).await)
}

pub async fn preview(state: AppState, body: Value) -> Result<Value, InprocError> {
    let req: transfer::PreviewShareRequest = parse_body(body)?;
    unwrap_transfer(transfer::preview_share_files(State(state), Json(req)).await)
}

pub async fn delete(state: AppState, task_id: String) -> Result<Value, InprocError> {
    unwrap_transfer(transfer::delete_transfer(State(state), Path(task_id)).await)
}

pub async fn cancel(state: AppState, task_id: String) -> Result<Value, InprocError> {
    unwrap_transfer(transfer::cancel_transfer(State(state), Path(task_id)).await)
}

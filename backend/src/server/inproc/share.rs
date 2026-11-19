//! 分享管理进程内门面。

use axum::extract::{Path, Query, State};
use axum::Json;
use serde_json::Value;

use super::{envelope, parse_body, InprocError};
use crate::server::handlers::share::{self, CancelShareRequest, ShareListQuery};
use crate::server::AppState;

pub async fn create(state: AppState, body: Value) -> Result<Value, InprocError> {
    let req: share::CreateShareRequest = parse_body(body)?;
    let Json(resp) = share::create_share(State(state), Json(req))
        .await
        .map_err(InprocError::status)?;
    envelope(resp.code, resp.message, resp.data)
}

pub async fn cancel(state: AppState, body: Value) -> Result<Value, InprocError> {
    let req: CancelShareRequest = parse_body(body)?;
    let Json(resp) = share::cancel_share(State(state), Json(req))
        .await
        .map_err(InprocError::status)?;
    envelope(resp.code, resp.message, resp.data)
}

pub async fn list(state: AppState, page: u32) -> Result<Value, InprocError> {
    let Json(resp) = share::get_share_list(State(state), Query(ShareListQuery { page }))
        .await
        .map_err(InprocError::status)?;
    envelope(resp.code, resp.message, resp.data)
}

pub async fn detail(state: AppState, share_id: u64) -> Result<Value, InprocError> {
    let Json(resp) = share::get_share_detail(State(state), Path(share_id))
        .await
        .map_err(InprocError::status)?;
    envelope(resp.code, resp.message, resp.data)
}

//! 分享同步（订阅）进程内门面。

use axum::extract::{Path, Query, State};
use axum::Json;
use serde_json::Value;

use super::{parse_body, success_envelope, InprocError};
use crate::server::handlers::share_sync::{self as ss, RunsQuery};
use crate::server::AppState;

fn unwrap_ss<T: serde::Serialize>(
    resp: ss::ApiResponse<T>,
) -> Result<Value, InprocError> {
    success_envelope(resp.success, resp.error, Some(resp.data))
}

fn api_err(e: crate::server::error::ApiError) -> InprocError {
    InprocError::from_api_error(&e)
}

pub async fn list_subscriptions(state: AppState) -> Result<Value, InprocError> {
    let Json(resp) = ss::list_subscriptions(
        State(state),
        Query(ss::ListQuery { uid: None }),
    )
    .await
    .map_err(api_err)?;
    unwrap_ss(resp)
}

pub async fn create_subscription(state: AppState, body: Value) -> Result<Value, InprocError> {
    let req: crate::share_sync::config::CreateShareSubscriptionRequest = parse_body(body)?;
    let Json(resp) = ss::create_subscription(State(state), Json(req))
        .await
        .map_err(api_err)?;
    unwrap_ss(resp)
}

pub async fn delete_subscription(state: AppState, id: String) -> Result<Value, InprocError> {
    let Json(resp) = ss::delete_subscription(State(state), Path(id))
        .await
        .map_err(api_err)?;
    unwrap_ss(resp)
}

pub async fn set_enabled(
    state: AppState,
    id: String,
    enabled: bool,
) -> Result<Value, InprocError> {
    let Json(resp) = if enabled {
        ss::enable_subscription(State(state), Path(id)).await
    } else {
        ss::disable_subscription(State(state), Path(id)).await
    }
    .map_err(api_err)?;
    unwrap_ss(resp)
}

pub async fn trigger(state: AppState, id: String) -> Result<Value, InprocError> {
    let Json(resp) = ss::trigger_subscription(State(state), Path(id))
        .await
        .map_err(api_err)?;
    unwrap_ss(resp)
}

pub async fn resume(state: AppState, id: String) -> Result<Value, InprocError> {
    let Json(resp) = ss::resume_subscription(State(state), Path(id))
        .await
        .map_err(api_err)?;
    unwrap_ss(resp)
}

pub async fn list_runs(state: AppState, id: String, page: usize) -> Result<Value, InprocError> {
    let Json(resp) = ss::list_runs(
        State(state),
        Path(id),
        Query(RunsQuery {
            page: Some(page),
            page_size: Some(20),
        }),
    )
    .await
    .map_err(api_err)?;
    unwrap_ss(resp)
}

pub async fn preview_tree(state: AppState, body: Value) -> Result<Value, InprocError> {
    let req: ss::PreviewTreeRequest = parse_body(body)?;
    let Json(resp) = ss::preview_tree(State(state), Json(req))
        .await
        .map_err(api_err)?;
    unwrap_ss(resp)
}

//! 本地文件系统浏览进程内门面。

use axum::extract::{Query, State};
use axum::Json;
use serde_json::{json, Value};

use super::{envelope, parse_body, InprocError};
use crate::filesystem::FsError;
use crate::server::handlers::filesystem;
use crate::server::AppState;

fn fs_error(e: FsError) -> InprocError {
    InprocError::new(e.code.code() as i64, e.message)
}

pub async fn list_directory(
    state: AppState,
    path: String,
    page: usize,
    page_size: usize,
    sort_field: String,
    sort_order: String,
) -> Result<Value, InprocError> {
    let req: crate::filesystem::ListRequest = parse_body(json!({
        "path": path,
        "page": page,
        "page_size": page_size,
        "sort_field": sort_field,
        "sort_order": sort_order,
    }))?;
    let Json(resp) = filesystem::list_directory(State(state), Query(req))
        .await
        .map_err(fs_error)?;
    envelope(resp.code, resp.message, resp.data)
}

pub async fn goto(state: AppState, path: String) -> Result<Value, InprocError> {
    let Json(resp) =
        filesystem::goto_path(State(state), Query(crate::filesystem::GotoRequest { path })).await;
    envelope(resp.code, resp.message, resp.data)
}

pub async fn roots(state: AppState) -> Result<Value, InprocError> {
    let Json(resp) = filesystem::get_roots(State(state)).await.map_err(fs_error)?;
    envelope(resp.code, resp.message, resp.data)
}

//! 本地下载目录浏览进程内门面。

use axum::extract::{Query, State};
use axum::Json;
use serde_json::{json, Value};

use super::{envelope, parse_body, InprocError};
use crate::server::handlers::local_files;
use crate::server::AppState;

pub async fn list(
    state: AppState,
    path: String,
    page: usize,
    page_size: usize,
    keyword: Option<String>,
) -> Result<Value, InprocError> {
    let req: local_files::LocalFileListQuery = parse_body(json!({
        "path": path,
        "page": page,
        "page_size": page_size,
        "keyword": keyword,
    }))?;
    let Json(resp) = local_files::list_local_files(State(state), Query(req))
        .await
        .map_err(InprocError::status)?;
    envelope(resp.code, resp.message, resp.data)
}

pub async fn delete(state: AppState, paths: Vec<String>) -> Result<Value, InprocError> {
    let Json(resp) = local_files::delete_local_files(
        State(state),
        Json(local_files::DeleteLocalFilesRequest { paths }),
    )
    .await
    .map_err(InprocError::status)?;
    envelope(resp.code, resp.message, resp.data)
}

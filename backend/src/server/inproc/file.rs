//! 网盘文件进程内门面。

use axum::extract::{Query, State};
use axum::Json;
use serde_json::Value;

use super::{envelope, parse_body, InprocError};
use crate::server::handlers::file::{self, FileListQuery, SearchQuery};
use crate::server::AppState;

pub async fn list(
    state: AppState,
    dir: String,
    page: u32,
    page_size: u32,
) -> Result<Value, InprocError> {
    let Json(resp) = file::get_file_list(State(state), Query(FileListQuery { dir, page, page_size }))
        .await
        .map_err(InprocError::status)?;
    envelope(resp.code, resp.message, resp.data)
}

pub async fn search(
    state: AppState,
    key: String,
    page: u32,
    num: u32,
    recursion: i32,
) -> Result<Value, InprocError> {
    let Json(resp) = file::search_files(
        State(state),
        Query(SearchQuery {
            key,
            page,
            num,
            recursion,
        }),
    )
    .await
    .map_err(InprocError::status)?;
    envelope(resp.code, resp.message, resp.data)
}

pub async fn create_folder(state: AppState, path: String) -> Result<Value, InprocError> {
    let Json(resp) = file::create_folder(
        State(state),
        Json(file::CreateFolderRequest { path }),
    )
    .await
    .map_err(InprocError::status)?;
    envelope(resp.code, resp.message, resp.data)
}

pub async fn delete_files(state: AppState, paths: Vec<String>) -> Result<Value, InprocError> {
    let Json(resp) = file::delete_files(
        State(state),
        Json(file::DeleteFilesRequest { paths }),
    )
    .await
    .map_err(InprocError::status)?;
    envelope(resp.code, resp.message, resp.data)
}

pub async fn copy_files(state: AppState, body: Value) -> Result<Value, InprocError> {
    let req: file::CopyFilesRequest = parse_body(body)?;
    let Json(resp) = file::copy_files(State(state), Json(req))
        .await
        .map_err(InprocError::status)?;
    envelope(resp.code, resp.message, resp.data)
}

pub async fn move_files(state: AppState, body: Value) -> Result<Value, InprocError> {
    let req: file::MoveFilesRequest = parse_body(body)?;
    let Json(resp) = file::move_files(State(state), Json(req))
        .await
        .map_err(InprocError::status)?;
    envelope(resp.code, resp.message, resp.data)
}

pub async fn rename_file(state: AppState, body: Value) -> Result<Value, InprocError> {
    let req: file::RenameFileRequest = parse_body(body)?;
    let Json(resp) = file::rename_file(State(state), Json(req))
        .await
        .map_err(InprocError::status)?;
    envelope(resp.code, resp.message, resp.data)
}

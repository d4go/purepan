//! 本地文件(下载目录)浏览 API(移植 frontend/src/api/localFiles.ts)。

use reqwest::Method;
use serde::Deserialize;
use serde_json::json;

use super::filesystem::ListDirectoryResponse;
use super::{ApiError, ApiClient};

#[derive(Debug, Clone, Deserialize)]
pub struct DeleteLocalFilesData {
    pub deleted_count: usize,
    pub failed_paths: Vec<String>,
}

fn urlencode(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b'/' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

impl ApiClient {
    /// GET /local-files(默认下载目录)
    pub async fn list_local_files(
        &self,
        path: &str,
        page: usize,
        page_size: usize,
    ) -> Result<ListDirectoryResponse, ApiError> {
        self.request(
            Method::GET,
            &format!("/local-files?path={}&page={page}&page_size={page_size}", urlencode(path)),
            None,
        )
        .await
    }

    /// POST /local-files/delete(不可逆)
    pub async fn delete_local_files(&self, paths: &[String]) -> Result<DeleteLocalFilesData, ApiError> {
        self.request(Method::POST, "/local-files/delete", Some(json!({ "paths": paths }))).await
    }
}
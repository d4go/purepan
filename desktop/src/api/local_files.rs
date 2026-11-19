//! 本地文件(下载目录)浏览 API(进程内直调)。

use serde::Deserialize;

use purepan_backend::server::inproc;

use super::filesystem::ListDirectoryResponse;
use super::{ApiClient, ApiError};

#[derive(Debug, Clone, Deserialize)]
pub struct DeleteLocalFilesData {
    pub deleted_count: usize,
    pub failed_paths: Vec<String>,
}

impl ApiClient {
    pub async fn list_local_files(
        &self,
        path: &str,
        page: usize,
        page_size: usize,
        keyword: &str,
    ) -> Result<ListDirectoryResponse, ApiError> {
        let path = path.to_string();
        let keyword = (!keyword.trim().is_empty()).then(|| keyword.to_string());
        self.request(move |s| inproc::local_files::list(s, path, page, page_size, keyword))
            .await
    }

    pub async fn delete_local_files(&self, paths: &[String]) -> Result<DeleteLocalFilesData, ApiError> {
        let paths = paths.to_vec();
        self.request(move |s| inproc::local_files::delete(s, paths))
            .await
    }
}

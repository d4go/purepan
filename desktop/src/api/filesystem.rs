//! 本地文件系统浏览 API(移植 frontend/src/api/filesystem.ts)。
//! 后端线缆字段为 camelCase(entryType/currentPath/...),桌面侧镜像对齐。

use reqwest::Method;
use serde::Deserialize;

use super::{ApiError, ApiClient};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EntryType {
    File,
    Directory,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FileEntry {
    pub id: String,
    pub name: String,
    #[serde(rename = "entryType")]
    pub entry_type: EntryType,
    pub size: Option<u64>,
    #[serde(rename = "createdAt")]
    pub created_at: String,
    #[serde(rename = "updatedAt")]
    pub updated_at: String,
    pub icon: Option<String>,
    pub path: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PermissionInfo {
    #[serde(rename = "dirOwner")]
    pub dir_owner: String,
    #[serde(rename = "dirGroup")]
    pub dir_group: String,
    #[serde(rename = "processUser")]
    pub process_user: String,
    #[serde(rename = "processGroup")]
    pub process_group: String,
    #[serde(rename = "canWrite")]
    pub can_write: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ListDirectoryResponse {
    pub entries: Vec<FileEntry>,
    #[serde(rename = "currentPath")]
    pub current_path: String,
    #[serde(rename = "parentPath")]
    pub parent_path: Option<String>,
    pub total: usize,
    pub page: usize,
    #[serde(rename = "pageSize")]
    pub page_size: usize,
    #[serde(rename = "hasMore")]
    pub has_more: bool,
    #[serde(rename = "permissionInfo")]
    pub permission_info: Option<PermissionInfo>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RootsResponse {
    pub roots: Vec<FileEntry>,
    #[serde(rename = "defaultPath")]
    pub default_path: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GotoResponse {
    pub valid: bool,
    #[serde(rename = "resolvedPath")]
    pub resolved_path: String,
    #[serde(rename = "entryType")]
    pub entry_type: Option<EntryType>,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ValidateResponse {
    pub valid: bool,
    pub exists: bool,
    #[serde(rename = "entryType")]
    pub entry_type: Option<EntryType>,
    pub message: Option<String>,
}

/// FS 错误码 → 友好提示(移植 getFriendlyErrorMessage)。
pub fn friendly_error(code: i64, fallback: &str) -> String {
    match code {
        50001 => "该路径不在允许访问的范围内",
        50002 => "目录不存在",
        50003 => "没有权限访问该路径",
        50004 => "不允许访问符号链接",
        50005 => "读取目录失败,请稍后重试",
        50006 => "路径格式无效",
        50007 => "非法路径访问",
        50008 => "文件不存在",
        50009 => "指定路径不是目录",
        50010 => "指定路径不是文件",
        _ => fallback,
    }
    .to_string()
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
    /// GET /fs/list
    pub async fn list_directory(
        &self,
        path: &str,
        page: usize,
        page_size: usize,
        sort_field: &str,
        sort_order: &str,
        keyword: Option<&str>,
    ) -> Result<ListDirectoryResponse, ApiError> {
        let mut url = format!(
            "/fs/list?path={}&page={page}&page_size={page_size}&sort_field={sort_field}&sort_order={sort_order}",
            urlencode(path)
        );
        if let Some(k) = keyword {
            url.push_str(&format!("&keyword={}", urlencode(k)));
        }
        self.request(Method::GET, &url, None).await
    }

    /// GET /fs/goto
    pub async fn goto_path(&self, path: &str) -> Result<GotoResponse, ApiError> {
        self.request(Method::GET, &format!("/fs/goto?path={}", urlencode(path)), None).await
    }

    /// GET /fs/validate
    pub async fn validate_path(&self, path: &str, entry_type: Option<EntryType>) -> Result<ValidateResponse, ApiError> {
        let mut url = format!("/fs/validate?path={}", urlencode(path));
        if let Some(t) = entry_type {
            url.push_str(match t {
                EntryType::File => "&type=file",
                EntryType::Directory => "&type=directory",
            });
        }
        self.request(Method::GET, &url, None).await
    }

    /// GET /fs/roots
    pub async fn get_roots(&self) -> Result<RootsResponse, ApiError> {
        self.request(Method::GET, "/fs/roots", None).await
    }
}
//! 网盘文件 API(移植 frontend/src/api/file.ts)。

use reqwest::Method;
use serde::Deserialize;
use serde_json::json;

use super::{ApiError, ApiClient};

pub use purepan_backend::netdisk::FileItem;

/// 带加密信息的文件项(后端 FileItemWithEncryption 的桌面侧镜像,flatten 展开)。
#[derive(Debug, Clone, Deserialize)]
pub struct FileItemWithEncryption {
    #[serde(flatten)]
    pub file: FileItem,
    pub is_encrypted: bool,
    pub is_encrypted_folder: bool,
    pub original_name: Option<String>,
    pub original_size: Option<u64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FileListData {
    pub list: Vec<FileItemWithEncryption>,
    pub dir: String,
    pub page: u32,
    pub total: u64,
    pub has_more: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DownloadUrlData {
    pub fs_id: u64,
    pub url: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateFolderData {
    pub fs_id: u64,
    pub path: String,
    pub isdir: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DeleteFilesData {
    pub deleted_count: u32,
    pub failed_paths: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SearchData {
    pub list: Vec<FileItemWithEncryption>,
    pub has_more: bool,
}

/// copy/move 单条 item(对应后端 FileOperationItem)。
#[derive(Debug, Clone, serde::Serialize)]
pub struct FileOperationItem {
    pub path: String,
    pub dest: String,
    pub newname: String,
}

/// rename 单条 item(对应后端 RenameItem)。
#[derive(Debug, Clone, serde::Serialize)]
pub struct RenameItem {
    pub path: String,
    pub newname: String,
    pub id: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FileOperationResultItem {
    pub from: String,
    pub to: String,
}

/// 文件管理操作统一结果(kind 区分成败,HTTP 始终 200)。
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum FileOperationOutcome {
    Success {
        taskid: u64,
        total: u32,
        list: Vec<FileOperationResultItem>,
    },
    Failed {
        message: String,
        taskid: u64,
        errno: Option<i64>,
        task_errno: Option<i64>,
        authwidget: Option<serde_json::Value>,
        verify_scene: Option<i64>,
        #[serde(default)]
        still_running: bool,
    },
}

impl ApiClient {
    /// GET /files?dir=&page=&page_size=
    pub async fn file_list(
        &self,
        dir: &str,
        page: u32,
        page_size: u32,
    ) -> Result<FileListData, ApiError> {
        self.request(
            Method::GET,
            &format!(
                "/files?dir={}&page={page}&page_size={page_size}",
                urlencode(dir)
            ),
            None,
        )
        .await
    }

    /// GET /files/download?fs_id=
    pub async fn download_url(&self, fs_id: u64) -> Result<String, ApiError> {
        let data: DownloadUrlData = self
            .request(Method::GET, &format!("/files/download?fs_id={fs_id}"), None)
            .await?;
        Ok(data.url)
    }

    /// POST /files/folder
    pub async fn create_folder(&self, path: &str) -> Result<CreateFolderData, ApiError> {
        self.request(Method::POST, "/files/folder", Some(json!({ "path": path })))
            .await
    }

    /// POST /files/delete(业务错误码经 ApiError::Business.code 透出,如 errno 132 风控)
    pub async fn delete_files(&self, paths: &[String]) -> Result<DeleteFilesData, ApiError> {
        self.request(Method::POST, "/files/delete", Some(json!({ "paths": paths })))
            .await
    }

    /// GET /files/search?key=&page=&num=&recursion=
    pub async fn search_files(
        &self,
        key: &str,
        page: u32,
        num: u32,
        recursion: u32,
    ) -> Result<SearchData, ApiError> {
        self.request(
            Method::GET,
            &format!(
                "/files/search?key={}&page={page}&num={num}&recursion={recursion}",
                urlencode(key)
            ),
            None,
        )
        .await
    }

    /// POST /files/copy(长轮询,后端最多 ~310s;客户端超时需足够长)
    pub async fn copy_files(
        &self,
        items: &[FileOperationItem],
    ) -> Result<FileOperationOutcome, ApiError> {
        self.request_long(Method::POST, "/files/copy", Some(json!({ "items": items })))
            .await
    }

    /// POST /files/move
    pub async fn move_files(
        &self,
        items: &[FileOperationItem],
    ) -> Result<FileOperationOutcome, ApiError> {
        self.request_long(Method::POST, "/files/move", Some(json!({ "items": items })))
            .await
    }

    /// POST /files/rename
    pub async fn rename_file(&self, item: &RenameItem) -> Result<FileOperationOutcome, ApiError> {
        self.request_long(Method::POST, "/files/rename", Some(json!({ "item": item })))
            .await
    }
}

/// 文件名校验(与后端 validate_filename 完全一致,移植 file.ts validateFilename)。
/// 返回 None 通过;Some(错误描述) 失败。
pub fn validate_filename(name: &str) -> Option<String> {
    if name.is_empty() {
        return Some("名称不能为空".into());
    }
    // UTF-16 code units 长度,与后端 encode_utf16().count() 对齐
    if name.encode_utf16().count() > 255 {
        return Some("名称过长(最长 255 个字符)".into());
    }
    if name == "." || name == ".." {
        return Some(format!("名称不能为 {name}"));
    }
    if let Some(c) = name.chars().find(|c| matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|')) {
        return Some(format!("名称含非法字符: {c}"));
    }
    if name.chars().any(|c| (c as u32) < 0x20 || c as u32 == 0x7f) {
        return Some("名称含控制字符".into());
    }
    if name.starts_with(' ') || name.ends_with(' ') {
        return Some("名称不能以空格开头或结尾".into());
    }
    if name.ends_with('.') {
        return Some("名称不能以 . 结尾".into());
    }
    let stem = name.split('.').next().unwrap_or(name).to_uppercase();
    const RESERVED: &[&str] = &[
        "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7",
        "COM8", "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
    ];
    if RESERVED.contains(&stem.as_str()) {
        return Some(format!("名称不能使用系统保留词: {stem}"));
    }
    None
}

/// 路径归一化:去尾斜杠(除根),保留 leading /。
pub fn normalize_path(p: &str) -> String {
    if p.is_empty() {
        return String::new();
    }
    if p == "/" {
        return "/".into();
    }
    let trimmed = p.trim_end_matches('/');
    if trimmed.is_empty() { "/".into() } else { trimmed.into() }
}

pub fn join_path(parent: &str, name: &str) -> String {
    let base = normalize_path(parent);
    if base == "/" {
        format!("/{name}")
    } else {
        format!("{base}/{name}")
    }
}

pub fn basename(p: &str) -> String {
    let norm = normalize_path(p);
    match norm.rfind('/') {
        Some(idx) => norm[idx + 1..].to_string(),
        None => norm,
    }
}

pub fn dirname(p: &str) -> String {
    let norm = normalize_path(p);
    match norm.rfind('/') {
        Some(idx) if idx > 0 => norm[..idx].to_string(),
        _ => "/".into(),
    }
}

fn urlencode(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}
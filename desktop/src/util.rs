//! 通用工具(后续逐步移植 frontend/src/utils/fileUtils.ts 的格式化函数)。

use std::sync::{Arc, OnceLock};

use base64::Engine as _;
use gpui_kit::RenderImage;

/// 应用图标原始像素(窗口图标,Linux/X11 使用)。
pub fn app_icon() -> Arc<image::RgbaImage> {
    static ICON: OnceLock<Arc<image::RgbaImage>> = OnceLock::new();
    ICON.get_or_init(|| {
        let img = image::load_from_memory(include_bytes!("../assets/icons/icon.png"))
            .expect("内置应用图标损坏")
            .to_rgba8();
        Arc::new(img)
    })
    .clone()
}

/// 应用 logo(GPUI 可渲染),用于标题栏。
pub fn app_logo() -> Arc<RenderImage> {
    static LOGO: OnceLock<Arc<RenderImage>> = OnceLock::new();
    LOGO.get_or_init(|| {
        let img = image::load_from_memory(include_bytes!("../assets/icons/icon.png"))
            .expect("内置应用图标损坏")
            .to_rgba8();
        Arc::new(RenderImage::new(vec![image::Frame::new(img)]))
    })
    .clone()
}

/// 把后端返回的 `data:image/png;base64,...` 解码为 GPUI 可渲染的图像。
pub fn decode_data_uri_image(data_uri: &str) -> Option<Arc<RenderImage>> {
    let b64 = data_uri.split_once("base64,")?.1;
    let bytes = base64::engine::general_purpose::STANDARD.decode(b64).ok()?;
    let img = image::load_from_memory(&bytes).ok()?.to_rgba8();
    Some(Arc::new(RenderImage::new(vec![image::Frame::new(img)])))
}

/// 从任意文本中提取百度网盘分享链接与提取码。
/// 支持 `https://pan.baidu.com/s/xxx?pwd=abcd` 及「链接 + 提取码: abcd」两种复制格式。
pub fn parse_share_link(text: &str) -> Option<(String, Option<String>)> {
    let lower = text.to_lowercase();
    let idx = ["pan.baidu.com/s/", "yun.baidu.com/s/"]
        .iter()
        .filter_map(|p| lower.find(p))
        .min()?;
    let mut start = idx;
    if text[..idx].ends_with("https://") {
        start = idx - 8;
    } else if text[..idx].ends_with("http://") {
        start = idx - 7;
    }
    let rest = &text[start..];
    let end = rest
        .find(|c: char| {
            c.is_whitespace()
                || matches!(c, '"' | '\'' | '<' | '>' | '，' | '。' | '；' | '！' | '？' | '、' | ')' | '）' | ']' | '】')
        })
        .unwrap_or(rest.len());
    let mut url = rest[..end].trim_end_matches(['.', ',', ';', ':', '!']).to_string();
    if url.is_empty() {
        return None;
    }
    if !url.starts_with("http") {
        url = format!("https://{url}");
    }

    let url_lower = url.to_lowercase();
    let mut pwd = None;
    for key in ["pwd=", "password="] {
        if let Some(pos) = url_lower.find(key) {
            let code: String = url[pos + key.len()..]
                .chars()
                .take(4)
                .take_while(|c| c.is_ascii_alphanumeric())
                .collect();
            if code.len() == 4 {
                pwd = Some(code);
            }
            break;
        }
    }
    if pwd.is_none() {
        if let Some(pos) = lower.find("提取码") {
            let code: String = lower[pos..]
                .chars()
                .skip(3)
                .take(12)
                .filter(|c| c.is_ascii_alphanumeric())
                .take(4)
                .collect();
            if code.len() == 4 {
                pwd = Some(code);
            }
        }
    }
    Some((url, pwd))
}

/// 字节数 → 人类可读(移植 fileUtils.ts formatSize)。
pub fn format_size(bytes: u64) -> String {
    const UNITS: [&str; 6] = ["B", "KB", "MB", "GB", "TB", "PB"];
    if bytes == 0 {
        return "0 B".into();
    }
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} B")
    } else {
        format!("{value:.2} {}", UNITS[unit])
    }
}

/// 速度(移植 formatSpeed)。
pub fn format_speed(bytes_per_sec: u64) -> String {
    format!("{}/s", format_size(bytes_per_sec))
}

/// 剩余时间(移植 formatETA):None→计算中,0→即将完成。
pub fn format_eta(seconds: Option<u64>) -> String {
    match seconds {
        None => "计算中".into(),
        Some(0) => "即将完成".into(),
        Some(s) => {
            let h = s / 3600;
            let m = (s % 3600) / 60;
            let sec = s % 60;
            if h > 0 {
                format!("{h}小时{m}分")
            } else if m > 0 {
                format!("{m}分{sec}秒")
            } else {
                format!("{sec}秒")
            }
        }
    }
}

/// Unix 秒 → 本地时间字符串(移植 formatTimestamp,0 → "-")。
pub fn format_timestamp(ts: i64) -> String {
    if ts == 0 {
        return "-".into();
    }
    chrono::DateTime::from_timestamp(ts, 0)
        .map(|dt| {
            dt.with_timezone(&chrono::Local)
                .format("%Y/%m/%d %H:%M:%S")
                .to_string()
        })
        .unwrap_or_else(|| "-".into())
}

/// 扩展名 → 中文类型(移植 FilesView.getFileType)。
pub fn file_type(filename: &str) -> &'static str {
    let ext = filename.rsplit('.').next().unwrap_or("").to_lowercase();
    match ext.as_str() {
        "jpg" | "jpeg" | "png" | "gif" | "webp" | "svg" => "图片",
        "mp4" | "avi" | "mov" | "mkv" | "flv" => "视频",
        "mp3" | "wav" | "flac" | "aac" => "音频",
        "pdf" => "PDF",
        "doc" | "docx" => "文档",
        "txt" | "md" => "文本",
        "zip" | "rar" | "7z" | "tar" | "gz" => "压缩",
        "exe" | "msi" | "app" => "程序",
        "js" | "ts" | "py" | "java" | "cpp" => "代码",
        _ => "文件",
    }
}

/// 文件显示名:加密文件/文件夹显示原始名(移植 getDisplayName)。
pub fn display_name(file: &crate::api::file::FileItemWithEncryption) -> String {
    if (file.is_encrypted || file.is_encrypted_folder) {
        if let Some(name) = &file.original_name {
            return name.clone();
        }
    }
    file.file.server_filename.clone()
}
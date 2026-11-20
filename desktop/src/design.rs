//! 极简云盘视觉规范的设计 token(颜色 / 阴影 / 图标底色)。
//! 亮暗两套取自设计稿,组件库主题仍负责原生控件外观。

use gpui_kit::assets::IconName;
use gpui_kit::component::ActiveTheme as _;
use gpui_kit::{
    App, Background, BoxShadow, Hsla, linear_color_stop, linear_gradient, px, rgb,
};

pub struct Palette {
    pub canvas: Hsla,
    /// 窗口 chrome 底色(实色):sidebar 叠加 canvas 的合成色,
    /// 主窗顶栏/侧栏/内容区外沿统一用它,内容面板浮于其上。
    pub chrome: Hsla,
    pub sidebar: Hsla,
    pub card: Hsla,
    pub border: Hsla,
    pub text: Hsla,
    pub muted: Hsla,
    pub faint: Hsla,
    pub primary: Hsla,
    pub primary_soft: Hsla,
    pub selected_bg: Hsla,
    pub danger: Hsla,
    pub is_dark: bool,
}

impl Palette {
    fn light() -> Self {
        Self {
            canvas: rgb(0xFCFDFF).into(),
            chrome: rgb(0xF6F8FD).into(),
            sidebar: rgb(0xF4F7FC).alpha(0.78).into(),
            card: rgb(0xFFFFFF).into(),
            border: rgb(0x0F172A).alpha(0.06).into(),
            text: rgb(0x1F2937).into(),
            muted: rgb(0x6B7280).into(),
            faint: rgb(0x9CA3AF).into(),
            primary: rgb(0x3B82F6).into(),
            primary_soft: rgb(0x3B82F6).alpha(0.12).into(),
            selected_bg: rgb(0x3B82F6).alpha(0.04).into(),
            danger: rgb(0xEF4444).into(),
            is_dark: false,
        }
    }

    fn dark() -> Self {
        Self {
            canvas: rgb(0x16181D).into(),
            chrome: rgb(0x1B1D21).into(),
            sidebar: rgb(0x1C1E22).alpha(0.85).into(),
            card: rgb(0x20232A).into(),
            border: rgb(0xFFFFFF).alpha(0.08).into(),
            text: rgb(0xE5E7EB).into(),
            muted: rgb(0x9CA3AF).into(),
            faint: rgb(0x6B7280).into(),
            primary: rgb(0x3B82F6).into(),
            primary_soft: rgb(0x3B82F6).alpha(0.18).into(),
            selected_bg: rgb(0x3B82F6).alpha(0.10).into(),
            danger: rgb(0xF87171).into(),
            is_dark: true,
        }
    }

    /// 卡片投影:默认极轻,悬停加深。
    pub fn card_shadow(&self, hovered: bool) -> Vec<BoxShadow> {
        let (dy, blur, alpha) = if hovered {
            (10.0, 28.0, if self.is_dark { 0.45 } else { 0.07 })
        } else {
            (4.0, 20.0, if self.is_dark { 0.35 } else { 0.03 })
        };
        vec![
            BoxShadow::new(px(0.), px(dy), rgb(0x0F172A).alpha(alpha).into())
                .blur_radius(px(blur)),
        ]
    }

    /// 文件夹图标底色:暖阳黄 / 重要文件夹冰川蓝。
    pub fn folder_tile(&self, important: bool) -> Background {
        if important {
            linear_gradient(
                135.,
                linear_color_stop(rgb(0xBFDBFE), 0.),
                linear_color_stop(rgb(0x60A5FA), 1.),
            )
        } else {
            linear_gradient(
                135.,
                linear_color_stop(rgb(0xFDE68A), 0.),
                linear_color_stop(rgb(0xF59E0B), 1.),
            )
        }
    }
}

pub fn palette(cx: &App) -> Palette {
    if cx.theme().is_dark() {
        Palette::dark()
    } else {
        Palette::light()
    }
}

/// 文件类型 → (图标, 柔和底色, 前景色)。下载页与全部文件页共用,
/// 保证视频/音乐/图片/压缩包/程序/表格/PDF/文档/代码等图标与配色一致。
/// 暗色主题下底色退化为前景色的低透明度。
pub fn file_visual(filename: &str, is_dark: bool) -> (IconName, Hsla, Hsla) {
    let ext = filename.rsplit('.').next().unwrap_or("").to_lowercase();
    let (icon, bg, fg): (IconName, u32, u32) = match ext.as_str() {
        "mp4" | "avi" | "mov" | "mkv" | "flv" | "webm" => {
            (IconName::FileVideoCamera, 0xEDE9FE, 0x7C3AED)
        }
        "zip" | "rar" | "7z" | "tar" | "gz" => (IconName::FileArchive, 0xDBEAFE, 0x2563EB),
        "exe" | "msi" | "app" | "apk" | "dmg" | "deb" | "rpm" | "pkg" | "ipa" | "appx"
        | "msix" => (IconName::AppWindow, 0xFFEDD5, 0xEA580C),
        "pdf" => (IconName::FileText, 0xFEE2E2, 0xDC2626),
        "xls" | "xlsx" | "csv" => (IconName::FileSpreadsheet, 0xD1FAE5, 0x059669),
        "jpg" | "jpeg" | "png" | "gif" | "webp" | "svg" | "bmp" => {
            (IconName::FileImage, 0xFCE7F3, 0xDB2777)
        }
        "mp3" | "wav" | "flac" | "aac" | "m4a" => (IconName::FileMusic, 0xE0F2FE, 0x0284C7),
        "js" | "ts" | "py" | "java" | "cpp" | "rs" | "go" | "json" => {
            (IconName::FileCode, 0xF3E8FF, 0x9333EA)
        }
        "doc" | "docx" | "txt" | "md" | "ppt" | "pptx" => {
            (IconName::FileText, 0xE0E7FF, 0x4F46E5)
        }
        _ => (IconName::FileText, 0xF1F5F9, 0x64748B),
    };
    let fg: Hsla = rgb(fg).into();
    let bg: Hsla = if is_dark { fg.alpha(0.16) } else { rgb(bg).into() };
    (icon, bg, fg)
}

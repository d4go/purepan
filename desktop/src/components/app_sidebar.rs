use gpui_kit::assets::IconName;
use gpui_kit::component::sidebar::{Sidebar, SidebarCollapsible, SidebarMenu, SidebarMenuItem};
use gpui_kit::{App, ClickEvent, Styled as _, Window, px};

/// 主导航(顺序与 Vue 版 MainLayout 一致)。
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Nav {
    Files,
    Local,
    Downloads,
    Uploads,
    Transfers,
    CloudDl,
    ShareSync,
    AutoBackup,
    Shares,
    Settings,
}

impl Nav {
    pub const ALL: &'static [Nav] = &[
        Nav::Files,
        Nav::Local,
        Nav::Downloads,
        Nav::Uploads,
        Nav::Transfers,
        Nav::CloudDl,
        Nav::ShareSync,
        Nav::AutoBackup,
        Nav::Shares,
        Nav::Settings,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Nav::Files => "网盘管理",
            Nav::Local => "本地文件",
            Nav::Downloads => "下载管理",
            Nav::Uploads => "上传管理",
            Nav::Transfers => "转存管理",
            Nav::CloudDl => "离线下载",
            Nav::ShareSync => "分享同步",
            Nav::AutoBackup => "自动备份",
            Nav::Shares => "分享管理",
            Nav::Settings => "系统设置",
        }
    }

    pub fn icon(self) -> IconName {
        match self {
            Nav::Files => IconName::Folder,
            Nav::Local => IconName::HardDrive,
            Nav::Downloads => IconName::Download,
            Nav::Uploads => IconName::Upload,
            Nav::Transfers => IconName::FolderInput,
            Nav::CloudDl => IconName::CloudDownload,
            Nav::ShareSync => IconName::FolderSync,
            Nav::AutoBackup => IconName::RefreshCw,
            Nav::Shares => IconName::Share2,
            Nav::Settings => IconName::Settings,
        }
    }
}

/// 构建侧栏。on_click 由调用方为每个 Nav 生成(通常是 cx.listener)。
pub fn build<G>(current: Nav, on_click: impl Fn(Nav) -> G) -> Sidebar<SidebarMenu>
where
    G: Fn(&ClickEvent, &mut Window, &mut App) + 'static,
{
    Sidebar::new("app-sidebar")
        .collapsible(SidebarCollapsible::None)
        .w(px(208.))
        .child(SidebarMenu::new().children(Nav::ALL.iter().map(|&nav| {
            SidebarMenuItem::new(nav.label())
                .icon(nav.icon())
                .active(current == nav)
                .on_click(on_click(nav))
        })))
}
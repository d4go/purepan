//! 云盘侧边栏:品牌、分组导航、设置与存储容量。
//! 导航项使用圆角整行高亮,选中时同步加深图标和文字。

use std::time::Duration;

use gpui_kit::assets::IconName;
use gpui_kit::component::spinner::Spinner;
use gpui_kit::component::{Icon, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    Animation, AnimationExt as _, App, ClickEvent, FontWeight, InteractiveElement as _, IntoElement,
    ParentElement as _, StatefulInteractiveElement as _, Styled as _, Window, div, ease_in_out, img,
    px, relative,
};
use purepan_backend::netdisk::StorageQuota;

use crate::design::Palette;
use crate::util;

/// 主导航页面。
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

const NAV_GROUPS: &[&[Nav]] = &[
    &[Nav::Files, Nav::Local],
    &[Nav::Downloads, Nav::Uploads, Nav::Transfers, Nav::CloudDl],
    &[Nav::Shares, Nav::ShareSync, Nav::AutoBackup],
];

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum StorageStatus {
    Loading,
    Ready(StorageQuota),
    Unavailable,
}

impl Nav {
    pub fn label(self) -> &'static str {
        match self {
            Nav::Files => "全部文件",
            Nav::Local => "本地文件",
            Nav::Downloads => "下载",
            Nav::Uploads => "上传",
            Nav::Transfers => "转存",
            Nav::CloudDl => "离线下载",
            Nav::ShareSync => "分享同步",
            Nav::AutoBackup => "自动备份",
            Nav::Shares => "分享管理",
            Nav::Settings => "设置",
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

pub fn build<G>(
    current: Nav,
    palette: &Palette,
    storage: StorageStatus,
    on_navigate: impl Fn(Nav) -> G,
) -> impl IntoElement
where
    G: Fn(&ClickEvent, &mut Window, &mut App) + 'static,
{
    let p = palette;
    let nav_hover_bg = p.text.alpha(0.05);
    let nav_selected_bg = p.text.alpha(0.09);

    let nav_item = |nav: Nav| {
        let selected = nav == current;
        let foreground = if selected { p.text } else { p.muted };
        let hover_bg = if selected { nav_selected_bg } else { nav_hover_bg };
        h_flex()
            .id(nav.label())
            .h(px(36.))
            .px(px(10.))
            .gap(px(10.))
            .rounded(px(8.))
            .cursor_pointer()
            .text_size(px(14.))
            .text_color(foreground)
            .hover(|s| s.bg(hover_bg))
            .when(selected, |d| d.bg(nav_selected_bg).font_weight(FontWeight::SEMIBOLD))
            .child(
                Icon::new(nav.icon())
                    .size(px(18.))
                    .flex_shrink_0()
                    .text_color(foreground),
            )
            .child(div().truncate().child(nav.label()))
            .on_click(on_navigate(nav))
    };

    let nav_groups = v_flex()
        .flex_1()
        .gap(px(14.))
        .children(NAV_GROUPS.iter().map(|group| {
            v_flex()
                .gap(px(2.))
                .children(group.iter().copied().map(&nav_item))
        }));

    v_flex()
        .w(px(220.))
        .h_full()
        .flex_shrink_0()
        .bg(p.sidebar)
        .px(px(16.))
        .py(px(24.))
        .gap(px(20.))
        .child(
            h_flex()
                .gap(px(10.))
                .items_center()
                .px(px(10.))
                .child(img(util::app_logo()).size(px(24.)).rounded(px(6.)))
                .child(
                    div()
                        .text_size(px(16.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(p.text)
                        .child("PurePan"),
                ),
        )
        .child(nav_groups)
        .child(
            v_flex()
                .gap(px(16.))
                .child(render_storage(storage, p))
                .child(nav_item(Nav::Settings)),
        )
}

fn render_storage(storage: StorageStatus, p: &Palette) -> impl IntoElement {
    let details = match storage {
        StorageStatus::Ready(quota) => {
            let available = quota.total.saturating_sub(quota.used);
            let ratio = if quota.total == 0 {
                0.
            } else {
                (quota.used.min(quota.total) as f64 / quota.total as f64) as f32
            };
            v_flex()
                .gap(px(6.))
                .child(
                    div().h(px(3.)).w_full().rounded_full().bg(p.border).child(
                        div()
                            .h_full()
                            .rounded_full()
                            .bg(p.primary)
                            .with_animation(
                                "storage-usage-fill",
                                Animation::new(Duration::from_millis(400)).with_easing(ease_in_out),
                                move |fill, progress| fill.w(relative(ratio * progress)),
                            ),
                    ),
                )
                .child(
                    h_flex()
                        .justify_between()
                        .gap(px(6.))
                        .child(format!("可用 {}", util::format_size(available)))
                        .child(format!("已用 {}", util::format_size(quota.used))),
                )
                .into_any_element()
        }
        StorageStatus::Loading => h_flex()
            .gap(px(6.))
            .child(Spinner::new().with_size(px(12.)).color(p.primary))
            .child("正在读取容量…")
            .into_any_element(),
        StorageStatus::Unavailable => div().child("暂无法获取容量").into_any_element(),
    };

    v_flex()
        .gap(px(6.))
        .px(px(10.))
        .text_size(px(11.))
        .text_color(p.muted)
        .child(div().font_weight(FontWeight::MEDIUM).child("网盘空间"))
        .child(details)
}

//! 任务列表通用视觉:状态胶囊、4px 进度条、圆形操作按钮与空状态引导。
//! 下载 / 上传 / 转存 / 离线下载 / 分享同步 / 自动备份 / 分享管理页共用。

use gpui_kit::assets::IconName;
use gpui_kit::component::tag::Tag;
use gpui_kit::component::{Icon, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AnyElement, Context, ElementId, Hsla, InteractiveElement as _, IntoElement,
    ParentElement as _, SharedString, StatefulInteractiveElement as _, Styled as _, Window, div, px,
    rgb,
};

use crate::design::Palette;

pub fn status_label(status: &str) -> &'static str {
    match status {
        "pending" => "等待中",
        "queued" => "排队中",
        "checking_share" => "检查分享",
        "downloading" => "下载中",
        "uploading" => "上传中",
        "checking_rapid" => "秒传检查",
        "scanning" => "扫描中",
        "decrypting" => "解密中",
        "encrypting" => "加密中",
        "cleaning" => "清理中",
        "preparing" => "准备中",
        "checking" => "检查中",
        "waiting_transfer" => "等待传输",
        "skipped" => "已跳过",
        "partially_completed" => "部分完成",
        "paused" => "已暂停",
        "completed" => "已完成",
        "rapid_upload_success" => "秒传成功",
        "transferring" => "转存中",
        "transferred" => "已转存",
        "transfer_failed" => "转存失败",
        "download_failed" => "下载失败",
        "failed" => "失败",
        "cancelled" => "已取消",
        "enabled" => "已启用",
        "disabled" => "已停用",
        _ => "未知",
    }
}

/// 覆盖所有任务类型(下载/上传/转存/文件夹/备份)的活跃状态集合。
pub fn is_active_status(status: &str) -> bool {
    matches!(
        status,
        "pending"
            | "queued"
            | "checking_share"
            | "downloading"
            | "uploading"
            | "decrypting"
            | "encrypting"
            | "scanning"
            | "checking_rapid"
            | "cleaning"
            | "preparing"
            | "checking"
            | "waiting_transfer"
            | "transferring"
    )
}

/// 柔和胶囊(浅底 + 同色文字)。
pub fn soft_pill(label: impl Into<SharedString>, bg: Hsla, fg: Hsla) -> Tag {
    Tag::custom(bg, fg, bg).small().rounded_full().child(label.into())
}

/// 状态胶囊:活跃淡蓝 / 暂停淡琥珀 / 完成淡绿 / 失败淡红 / 取消中性。
pub fn status_tag(status: &str, p: &Palette) -> Tag {
    let (bg, fg): (Hsla, Hsla) = match status {
        "paused" | "partially_completed" => {
            let fg: Hsla = if p.is_dark { rgb(0xFBBF24).into() } else { rgb(0xB45309).into() };
            (if p.is_dark { fg.alpha(0.16) } else { rgb(0xFEF3C7).into() }, fg)
        }
        "completed" | "transferred" | "rapid_upload_success" => {
            let fg: Hsla = if p.is_dark { rgb(0x34D399).into() } else { rgb(0x059669).into() };
            (if p.is_dark { fg.alpha(0.16) } else { rgb(0xD1FAE5).into() }, fg)
        }
        "failed" | "transfer_failed" | "download_failed" => {
            let fg = p.danger;
            (if p.is_dark { fg.alpha(0.16) } else { rgb(0xFEE2E2).into() }, fg)
        }
        "cancelled" => (p.selected_bg, p.muted),
        s if is_active_status(s) => (p.primary_soft, p.primary),
        _ => (p.selected_bg, p.muted),
    };
    soft_pill(status_label(status), bg, fg)
}

/// 4px 细圆角进度条。
pub fn progress_bar(pct: f32, fill: Hsla, track: Hsla) -> AnyElement {
    let pct = pct.clamp(0.0, 1.0);
    div()
        .h(px(4.))
        .w_full()
        .rounded_full()
        .bg(track)
        .child(div().h_full().rounded_full().bg(fill).w(gpui_kit::relative(pct)))
        .into_any_element()
}

/// 元信息行中的间隔点。
pub fn sep_dot(p: &Palette) -> AnyElement {
    div().text_size(px(11.)).text_color(p.faint.alpha(0.7)).child("·").into_any_element()
}

/// 轻量的圆形图标按钮。
pub fn circle_button<V: 'static>(
    p: &Palette,
    id: impl Into<ElementId>,
    icon: IconName,
    handler: impl Fn(&mut V, &mut Window, &mut Context<V>) + 'static,
    cx: &mut Context<V>,
) -> AnyElement {
    let hover_bg = p.selected_bg;
    let fg = p.muted;
    div()
        .id(id.into())
        .size(px(30.))
        .rounded_full()
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .hover(move |s| s.bg(hover_bg))
        .child(Icon::new(icon).size(px(15.)).text_color(fg))
        .on_click(cx.listener(move |this, _, window, cx| handler(this, window, cx)))
        .into_any_element()
}

/// 居中的空状态引导:柔和图标 + 主副文案 + 可选快捷操作。
pub fn empty_state(
    p: &Palette,
    icon: IconName,
    title: impl Into<SharedString>,
    subtitle: impl Into<SharedString>,
    action: Option<AnyElement>,
) -> AnyElement {
    v_flex()
        .flex_1()
        .items_center()
        .justify_center()
        .gap(px(12.))
        .child(
            div()
                .size(px(64.))
                .rounded(px(18.))
                .bg(p.primary_soft)
                .flex()
                .items_center()
                .justify_center()
                .child(Icon::new(icon).size(px(28.)).text_color(p.primary)),
        )
        .child(
            div()
                .text_size(px(15.))
                .font_weight(gpui_kit::FontWeight::MEDIUM)
                .child(title.into()),
        )
        .child(
            div()
                .text_size(px(12.))
                .text_color(p.faint)
                .child(subtitle.into()),
        )
        .when_some(action, |this, action| this.child(action))
        .into_any_element()
}

/// 卡片行外壳:左侧类型图标 + 中间信息 + 右侧操作。
pub fn task_card(p: &Palette, tile: AnyElement, mid: AnyElement, actions: AnyElement) -> AnyElement {
    h_flex()
        .gap(px(14.))
        .items_center()
        .px(px(16.))
        .py(px(12.))
        .rounded(px(14.))
        .bg(p.card)
        .border(px(1.))
        .border_color(p.border)
        .child(tile)
        .child(mid)
        .child(actions)
        .into_any_element()
}
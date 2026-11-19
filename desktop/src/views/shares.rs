//! 分享管理页(分享设置):总览头部 + 三段式分享卡片(分享图标、状态胶囊、
//! 链接与浏览数据、操作),查看提取码 / 复制链接 / 取消分享。

use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::dialog::Dialog;
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::{Icon, Sizable as _, WindowExt as _, h_flex, v_flex};
use gpui_kit::{
    AnyElement, AnyWindowHandle, ClipboardItem, Context, Entity,
    InteractiveElement as _, IntoElement, ParentElement as _, Render, Styled as _, Subscription,
    Window, div, px, rgb,
};

use crate::api::share::ShareRecordData;
use crate::api::{self};
use crate::bridge::{EventHub, ToastKind, UiEvent};
use crate::components::app_sidebar::Nav;
use crate::components::task_ui::{circle_button, empty_state, sep_dot, soft_pill, task_card};
use crate::components::toast;
use crate::design::{self, Palette};
use crate::state::Backend;

/// 有效期标签:永久 / 到期日期。
fn format_expiry(expired_type: i32, expired_time: i64) -> String {
    if expired_type == 0 {
        return "永久有效".into();
    }
    chrono::DateTime::from_timestamp(expired_time, 0)
        .map(|dt| format!("有效期 {}", dt.with_timezone(&chrono::Local).format("%Y/%m/%d")))
        .unwrap_or_else(|| "限时".into())
}

pub struct SharesView {
    hub: Entity<EventHub>,
    shares: Vec<ShareRecordData>,
    page: u32,
    total: u32,
    loading: bool,
    window_handle: AnyWindowHandle,
    _subscriptions: Vec<Subscription>,
}

impl SharesView {
    pub fn new(hub: Entity<EventHub>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let sub = cx.subscribe(&hub, |this, _, ev, cx| {
            if matches!(ev, UiEvent::AccountChanged { .. }) { this.refresh(cx); }
        });
        let mut v = Self { hub, shares: Vec::new(), page: 1, total: 0, loading: false, window_handle: window.window_handle(), _subscriptions: vec![sub] };
        v.refresh(cx);
        v
    }

    fn toast(&self, cx: &mut Context<Self>, kind: ToastKind, msg: impl Into<String>) {
        let msg = msg.into();
        let wh = self.window_handle;
        let _ = wh.update(cx, |_, w, cx| toast::show(w, cx, kind, msg.clone()));
    }

    fn refresh(&mut self, cx: &mut Context<Self>) {
        self.loading = true;
        let page = self.page;
        let b = cx.global::<Backend>();
        let (rt, api) = (b.rt.clone(), b.api.clone());
        cx.notify();
        cx.spawn(async move |this, cx| {
            let (a, p) = (api.clone(), page);
            let r = api::call_api(&rt, move || async move { a.list_shares(p).await }).await;
            let _ = this.update(cx, |this, cx| {
                this.loading = false;
                match r {
                    Ok(data) => { this.shares = data.list; this.total = data.total; }
                    Err(e) => this.toast(cx, ToastKind::Error, e.message()),
                }
                cx.notify();
            });
        }).detach();
    }

    fn copy(&self, text: String, window: &mut Window, cx: &mut Context<Self>) {
        cx.write_to_clipboard(ClipboardItem::new_string(text));
        toast::show(window, cx, ToastKind::Success, "已复制");
    }

    fn cancel(&mut self, share_id: u64, cx: &mut Context<Self>) {
        let b = cx.global::<Backend>();
        let (rt, api) = (b.rt.clone(), b.api.clone());
        cx.spawn(async move |this, cx| {
            let a = api.clone();
            let r = api::call_api(&rt, move || async move { a.cancel_share(&[share_id]).await }).await;
            let _ = this.update(cx, |this, cx| {
                match r {
                    Ok(_) => { this.toast(cx, ToastKind::Success, "已取消分享"); this.refresh(cx); }
                    Err(e) => this.toast(cx, ToastKind::Error, e.message()),
                }
            });
        }).detach();
    }

    fn show_detail(&mut self, share_id: u64, cx: &mut Context<Self>) {
        let b = cx.global::<Backend>();
        let (rt, api) = (b.rt.clone(), b.api.clone());
        let wh = self.window_handle;
        cx.spawn(async move |_, cx| {
            let a = api.clone();
            let r = api::call_api(&rt, move || async move { a.get_share_detail(share_id).await }).await;
            if let Ok(detail) = r {
                let _ = wh.update(cx, |_, window, cx| {
                    let pwd = detail.pwd.clone();
                    let url = detail.shorturl.clone();
                    let both = format!("{url} 提取码: {pwd}");
                    window.open_dialog(cx, move |d: Dialog, _, _| {
                        let pwd2 = pwd.clone();
                        let both2 = both.clone();
                        d.title("分享详情").width(px(420.))
                            .child(v_flex().gap_2()
                                .child(div().child(url.clone()))
                                .child(div().child(format!("提取码: {pwd}"))))
                            .footer(h_flex().gap_2().justify_end()
                                .child(Button::new("sd-copy").label("复制提取码").on_click(move |_, _, cx| {
                                    cx.write_to_clipboard(ClipboardItem::new_string(pwd2.clone()));
                                }))
                                .child(Button::new("sd-copy-both").primary().label("复制全部").on_click(move |_, _, cx| {
                                    cx.write_to_clipboard(ClipboardItem::new_string(both2.clone()));
                                })))
                    });
                });
            }
        }).detach();
    }

    fn render_row(&self, s: &ShareRecordData, p: &Palette, cx: &mut Context<Self>) -> AnyElement {
        let id = s.share_id;
        let link = s.shortlink.clone();
        let path = s.typical_path.clone();
        let expired = s.expired_type;
        let status = s.status;

        let tile = div()
            .size(px(44.))
            .rounded(px(12.))
            .bg(p.primary_soft)
            .flex()
            .items_center()
            .justify_center()
            .flex_shrink_0()
            .child(Icon::new(IconName::Share2).size(px(20.)).text_color(p.primary))
            .into_any_element();

        let mut title = h_flex().gap(px(8.)).items_center().min_w_0().overflow_hidden();
        title = title.child(
            div()
                .flex_1()
                .min_w_0()
                .truncate()
                .text_size(px(14.))
                .font_weight(gpui_kit::FontWeight::MEDIUM)
                .child(path),
        );
        title = title.child(if status == 0 {
            soft_pill(
                "正常",
                if p.is_dark { p.primary.alpha(0.16) } else { rgb(0xD1FAE5).into() },
                if p.is_dark { rgb(0x34D399).into() } else { rgb(0x059669).into() },
            )
        } else {
            soft_pill("已失效", if p.is_dark { p.danger.alpha(0.16) } else { rgb(0xFEE2E2).into() }, p.danger)
        });
        title = title.child(soft_pill(
            format_expiry(expired, s.expired_time),
            p.selected_bg,
            p.muted,
        ));

        let mut mid = v_flex().flex_1().min_w_0().gap(px(6.)).overflow_hidden().child(title);

        let mut meta = h_flex()
            .gap(px(6.))
            .items_center()
            .min_w_0()
            .overflow_hidden()
            .text_size(px(12.))
            .text_color(p.faint);
        meta = meta.child(div().truncate().child(link.clone()));
        meta = meta.child(sep_dot(p));
        meta = meta.child(format!("{} 次浏览", s.view_count));
        mid = mid.child(meta);

        let mut actions = h_flex().gap(px(2.)).items_center().flex_shrink_0();
        let call = |key: &str| gpui_kit::SharedString::from(format!("{key}:{id}"));
        actions = actions.child(circle_button(p, call("sh-pwd"), IconName::KeyRound, move |this, _, cx| {
            this.show_detail(id, cx)
        }, cx));
        let link_copy = link.clone();
        actions = actions.child(circle_button(p, call("sh-copy"), IconName::Copy, move |this, window, cx| {
            this.copy(link_copy.clone(), window, cx)
        }, cx));
        actions = actions.child(circle_button(p, call("sh-cancel"), IconName::Trash, move |this, _, cx| {
            this.cancel(id, cx)
        }, cx));

        task_card(p, tile, mid.into_any_element(), actions.into_any_element())
    }
}

impl Render for SharesView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = design::palette(cx);
        let count = if self.total > 0 { self.total } else { self.shares.len() as u32 };

        let status_pill = if count > 0 {
            soft_pill(format!("{count} 个分享"), p.primary_soft, p.primary)
        } else {
            soft_pill("暂无分享", p.selected_bg, p.muted)
        };

        let header = h_flex()
            .h(px(68.))
            .px(px(32.))
            .gap(px(12.))
            .items_center()
            .flex_shrink_0()
            .child(div().text_size(px(18.)).font_weight(gpui_kit::FontWeight::SEMIBOLD).child("分享管理"))
            .child(status_pill)
            .child(div().flex_1())
            .child(
                h_flex()
                    .gap(px(6.))
                    .items_center()
                    .child(
                        Button::new("sh-refresh")
                            .ghost()
                            .small()
                            .icon(IconName::RefreshCw)
                            .label("刷新")
                            .on_click(cx.listener(|this, _, _, cx| this.refresh(cx))),
                    ),
            )
            .into_any_element();

        let body: AnyElement = if self.shares.is_empty() {
            let loading = self.loading;
            let hub = self.hub.clone();
            empty_state(
                &p,
                IconName::Share2,
                if loading { "正在加载分享…" } else { "暂无分享" },
                if loading { "请稍候" } else { "在文件页选择文件后即可创建分享链接" },
                (!loading).then(|| {
                    Button::new("sh-goto-files")
                        .small()
                        .icon(IconName::Folder)
                        .label("去文件页")
                        .on_click(cx.listener(move |_, _, _, cx| {
                            hub.update(cx, |_, cx| cx.emit(UiEvent::Navigate(Nav::Files)));
                        }))
                        .into_any_element()
                }),
            )
        } else {
            let mut list = v_flex()
                .id("sh-scroll")
                .flex_1()
                .min_h_0()
                .gap(px(10.))
                .px(px(32.))
                .py(px(16.))
                .overflow_y_scrollbar();
            for s in &self.shares {
                list = list.child(self.render_row(s, &p, cx));
            }
            list.into_any_element()
        };

        v_flex()
            .flex_1()
            .h_full()
            .min_w_0()
            .overflow_hidden()
            .child(header)
            .child(body)
    }
}
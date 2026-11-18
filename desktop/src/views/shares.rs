//! 分享管理页(移植 SharesView.vue):分享列表,查看提取码/复制/取消。

use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::dialog::Dialog;
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::tag::Tag;
use gpui_kit::component::{ActiveTheme as _, Icon, Sizable as _, WindowExt as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AnyWindowHandle, App, AppContext as _, ClipboardItem, Context, Entity, IntoElement,
    ParentElement as _, Render, Styled as _, Subscription, Window, div, px,
};

use crate::api::share::ShareRecordData;
use crate::api::{self};
use crate::bridge::{EventHub, ToastKind, UiEvent};
use crate::components::toast;
use crate::state::Backend;
use crate::util;

pub struct SharesView {
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
        let mut v = Self { shares: Vec::new(), page: 1, total: 0, loading: false, window_handle: window.window_handle(), _subscriptions: vec![sub] };
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

    fn copy(&self, text: String, cx: &mut Context<Self>) {
        cx.write_to_clipboard(ClipboardItem::new_string(text));
        self.toast(cx, ToastKind::Success, "已复制");
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
}

impl Render for SharesView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let toolbar = h_flex().gap_2().items_center().px_4().py_2().border_b_1().border_color(theme.border)
            .child(div().flex_1().child(format!("分享管理 · {} 个", self.total)))
            .child(Button::new("sh-refresh").small().icon(IconName::RefreshCw).label("刷新").on_click(cx.listener(|this, _, _, cx| this.refresh(cx))));

        let mut list = v_flex().overflow_y_scrollbar();
        if self.shares.is_empty() {
            list = list.child(div().py_8().text_center().text_color(theme.muted_foreground).child(if self.loading { "加载中..." } else { "暂无分享" }));
        }
        for s in &self.shares {
            let id = s.share_id;
            let link = s.shortlink.clone();
            let path = s.typical_path.clone();
            let views = s.view_count;
            let expired = s.expired_type;
            let status = s.status;
            let id_detail = id;
            let id_cancel = id;
            let link_copy = link.clone();
            list = list.child(
                h_flex().gap_2().items_center().px_4().py_2().border_b_1().border_color(theme.border)
                    .child(Icon::new(IconName::Share2).size(px(18.)).text_color(theme.primary))
                    .child(v_flex().flex_1().gap_0p5()
                        .child(div().truncate().child(path))
                        .child(div().text_color(theme.muted_foreground).truncate().child(link)))
                    .child(div().text_color(theme.muted_foreground).child(format!("{views} 次浏览")))
                    .child(Tag::new().small().child(if status == 0 { "正常" } else { "失效" }))
                    .child(div().text_color(theme.muted_foreground).child(if expired == 0 { "永久".to_string() } else { util::format_timestamp(s.expired_time) }))
                    .child(Button::new(format!("sh-pwd-{id}")).ghost().small().icon(IconName::KeyRound).on_click(cx.listener(move |this, _, _, cx| this.show_detail(id_detail, cx))))
                    .child(Button::new(format!("sh-copy-{id}")).ghost().small().icon(IconName::Copy).on_click(cx.listener(move |this, _, _, cx| this.copy(link_copy.clone(), cx))))
                    .child(Button::new(format!("sh-cancel-{id}")).ghost().small().icon(IconName::Trash).on_click(cx.listener(move |this, _, _, cx| this.cancel(id_cancel, cx)))),
            );
        }
        v_flex().flex_1().overflow_hidden().child(toolbar).child(div().flex_1().child(list))
    }
}
//! 转存管理页(移植 TransfersView.vue):任务卡列表,新建转存/分享直下,取消/删除。

use std::rc::Rc;

use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::dialog::Dialog;
use gpui_kit::component::progress::Progress;
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::tag::Tag;
use gpui_kit::component::{ActiveTheme as _, Icon, Sizable as _, WindowExt as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AnyWindowHandle, App, AppContext as _, Context, Entity, IntoElement, ParentElement as _,
    Render, Styled as _, Subscription, Window, div, px,
};
use serde_json::Value;

use crate::api::{self, transfer::transfer_status_text};
use crate::bridge::{EventHub, ToastKind, UiEvent};
use crate::components::dialogs::transfer_dialog::TransferDialog;
use crate::components::toast;
use crate::state::task_cache::{v_bool, v_str, v_u64};
use crate::state::{Backend, TaskCache};

fn is_terminal(status: &str) -> bool {
    matches!(status, "transfer_failed" | "transferred" | "download_failed" | "completed")
}

pub struct TransfersView {
    window_handle: AnyWindowHandle,
    _subscriptions: Vec<Subscription>,
}

impl TransfersView {
    pub fn new(hub: Entity<EventHub>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let subs = vec![cx.subscribe(&hub, |_, _, ev, cx| {
            if matches!(ev, UiEvent::WsMessage(_) | UiEvent::AccountChanged { .. }) { cx.notify(); }
        })];
        cx.global::<Backend>().ws.request_snapshot();
        Self { window_handle: window.window_handle(), _subscriptions: subs }
    }

    fn toast(&self, cx: &mut Context<Self>, kind: ToastKind, msg: impl Into<String>) {
        let msg = msg.into();
        let wh = self.window_handle;
        let _ = wh.update(cx, |_, w, cx| toast::show(w, cx, kind, msg.clone()));
    }

    fn open_transfer_dialog(&mut self, direct: bool, cx: &mut Context<Self>) {
        let wh = self.window_handle;
        let initial = "/".to_string();
        let _ = wh.update(cx, move |_, window, cx| {
            let on_success: Rc<dyn Fn(&mut Window, &mut App)> = Rc::new(|_w, cx| {
                cx.global::<Backend>().ws.request_snapshot();
            });
            let dlg = cx.new(|cx| TransferDialog::new(direct, &initial, on_success, window, cx));
            let title = if direct { "分享直下" } else { "新建转存" };
            window.open_dialog(cx, move |d: Dialog, _, _| d.title(title).child(dlg.clone()));
        });
    }

    fn cancel(&mut self, id: &str, cx: &mut Context<Self>) {
        let b = cx.global::<Backend>();
        let (rt, api) = (b.rt.clone(), b.api.clone());
        let id = id.to_string();
        cx.spawn(async move |this, cx| {
            let (a, i) = (api.clone(), id.clone());
            let r = api::call_api(&rt, move || async move { a.cancel_transfer(&i).await }).await;
            let _ = this.update(cx, |this, cx| { if let Err(e) = r { this.toast(cx, ToastKind::Error, e.message()); } });
        }).detach();
    }

    fn delete(&mut self, id: &str, cx: &mut Context<Self>) {
        let b = cx.global::<Backend>();
        let (rt, api) = (b.rt.clone(), b.api.clone());
        let id = id.to_string();
        cx.spawn(async move |this, cx| {
            let (a, i) = (api.clone(), id.clone());
            let r = api::call_api(&rt, move || async move { a.delete_transfer(&i).await }).await;
            let _ = this.update(cx, |this, cx| { if let Err(e) = r { this.toast(cx, ToastKind::Error, e.message()); } });
        }).detach();
    }

    fn render_row(&self, t: &Value, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let id = v_str(t, "id");
        let status = v_str(t, "status");
        let title = {
            let name = v_str(t, "file_name");
            if name.is_empty() { v_str(t, "share_url") } else { name }
        };
        let total = v_u64(t, "total_count");
        let done = v_u64(t, "transferred_count");
        let pct = if total > 0 { (done as f64 / total as f64 * 100.0) as f32 } else { 0.0 };
        let error = v_str(t, "error");
        let terminal = is_terminal(&status);
        let id_cancel = id.clone();
        let id_del = id.clone();

        let mut row = v_flex().gap_1().px_4().py_2().border_b_1().border_color(theme.border);
        let mut top = h_flex().gap_2().items_center();
        top = top.child(Icon::new(IconName::FolderInput).size(px(18.)).text_color(theme.primary));
        top = top.child(div().flex_1().truncate().child(title));
        top = top.child(Tag::new().small().child(transfer_status_text(&status)));
        if v_bool(t, "auto_download") { top = top.child(Tag::new().small().child("自动下载")); }
        if v_bool(t, "is_share_direct_download") { top = top.child(Tag::new().small().child("直下")); }
        if !terminal {
            top = top.child(Button::new(format!("tf-cancel-{id}")).ghost().small().icon(IconName::CircleStop)
                .on_click(cx.listener(move |this, _, _, cx| this.cancel(&id_cancel, cx))));
        }
        top = top.child(Button::new(format!("tf-del-{id}")).ghost().small().icon(IconName::Trash)
            .on_click(cx.listener(move |this, _, _, cx| this.delete(&id_del, cx))));
        row = row.child(top);
        row = row.child(div().text_color(theme.muted_foreground).truncate().child(format!("保存至: {}", v_str(t, "save_path"))));
        row = row.child(Progress::new(format!("tf-prog-{id}")).value(pct));
        row = row.child(div().text_color(theme.muted_foreground).child(format!("{done}/{total} 文件")));
        if !error.is_empty() {
            row = row.child(div().text_color(theme.danger).child(error));
        }
        row
    }
}

impl Render for TransfersView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let mut rows: Vec<Value> = cx.global::<TaskCache>().transfers.values().cloned().collect();
        rows.sort_by(|a, b| v_u64(b, "created_at").cmp(&v_u64(a, "created_at")));

        let toolbar = h_flex().gap_2().items_center().px_4().py_2().border_b_1().border_color(theme.border)
            .child(div().flex_1().child("转存管理"))
            .child(Button::new("tf-new").primary().small().icon(IconName::FolderInput).label("新建转存").on_click(cx.listener(|this, _, _, cx| this.open_transfer_dialog(false, cx))))
            .child(Button::new("tf-direct").small().icon(IconName::Download).label("分享直下").on_click(cx.listener(|this, _, _, cx| this.open_transfer_dialog(true, cx))))
            .child(Button::new("tf-refresh").small().icon(IconName::RefreshCw).on_click(cx.listener(|_, _, _, cx| { cx.global::<Backend>().ws.request_snapshot(); })));

        let mut list = v_flex().overflow_y_scrollbar();
        if rows.is_empty() {
            list = list.child(div().py_8().text_center().text_color(theme.muted_foreground).child("暂无转存任务"));
        }
        for t in &rows {
            list = list.child(self.render_row(t, cx));
        }
        v_flex().flex_1().overflow_hidden().child(toolbar).child(div().flex_1().child(list))
    }
}
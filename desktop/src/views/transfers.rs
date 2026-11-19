//! 转存管理页:总览头部 + 三段式任务卡片(分享文件图标、进度、元信息、操作),
//! 新建转存 / 分享直下,取消/删除。数据来自 TaskCache(WS 驱动)。

use std::rc::Rc;

use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::dialog::Dialog;
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::tag::Tag;
use gpui_kit::component::{Icon, Sizable as _, WindowExt as _, h_flex, v_flex};
use gpui_kit::{
    AnyElement, AnyWindowHandle, App, AppContext as _, BorrowAppContext as _, Context, Entity,
    InteractiveElement as _, IntoElement, ParentElement as _, Render, Styled as _, Subscription,
    Window, div, px,
};
use serde_json::Value;

use crate::api::{self};
use crate::bridge::{EventHub, ToastKind, UiEvent};
use crate::components::dialogs::transfer_dialog::TransferDialog;
use crate::components::task_ui::{
    circle_button, empty_state, progress_bar, sep_dot, soft_pill, status_tag, task_card,
};
use crate::components::toast;
use crate::design::{self, Palette};
use crate::state::task_cache::{v_bool, v_str, v_u64};
use crate::state::{Backend, TaskCache};

fn is_terminal(status: &str) -> bool {
    matches!(status, "transfer_failed" | "transferred" | "download_failed" | "completed" | "cancelled" | "failed")
}

fn is_active(status: &str) -> bool {
    matches!(status, "pending" | "queued" | "checking_share" | "transferring" | "downloading" | "cleaning")
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

    fn open_transfer_dialog(&mut self, direct: bool, window: &mut Window, cx: &mut Context<Self>) {
        let initial = "/".to_string();
        let on_success: Rc<dyn Fn(&mut Window, &mut App)> = Rc::new(|_w, cx| {
            cx.global::<Backend>().ws.request_snapshot();
        });
        let dlg = cx.new(|cx| TransferDialog::new(direct, &initial, on_success, window, cx));
        let title = if direct { "分享直下" } else { "新建转存" };
        window.open_dialog(cx, move |d: Dialog, _, _| d.title(title).width(px(600.)).child(dlg.clone()));
    }

    fn cancel(&mut self, id: &str, cx: &mut Context<Self>) {
        let b = cx.global::<Backend>();
        let (rt, api) = (b.rt.clone(), b.api.clone());
        let id = id.to_string();
        cx.spawn(async move |this, cx| {
            let (a, i) = (api.clone(), id.clone());
            let r = api::call_api(&rt, move || async move { a.cancel_transfer(&i).await }).await;
            let _ = this.update(cx, |this, cx| {
                if let Err(e) = r { this.toast(cx, ToastKind::Error, e.message()); } else {
                    cx.global::<Backend>().ws.request_snapshot();
                    cx.notify();
                }
            });
        }).detach();
    }

    fn delete(&mut self, id: &str, cx: &mut Context<Self>) {
        let b = cx.global::<Backend>();
        let (rt, api) = (b.rt.clone(), b.api.clone());
        let id = id.to_string();
        cx.spawn(async move |this, cx| {
            let (a, i) = (api.clone(), id.clone());
            let r = api::call_api(&rt, move || async move { a.delete_transfer(&i).await }).await;
            let id = id.clone();
            let _ = this.update(cx, |this, cx| {
                match r {
                    Ok(_) => {
                        cx.update_global::<TaskCache, _>(|c, _| { c.transfers.remove(&id); });
                        cx.global::<Backend>().ws.request_snapshot();
                        cx.notify();
                    }
                    Err(e) => this.toast(cx, ToastKind::Error, e.message()),
                }
            });
        }).detach();
    }

    fn render_row(&self, t: &Value, p: &Palette, cx: &mut Context<Self>) -> AnyElement {
        let id = v_str(t, "id");
        let status = v_str(t, "status");
        let title_text = {
            let name = v_str(t, "file_name");
            if name.is_empty() { v_str(t, "share_url") } else { name }
        };
        let total = v_u64(t, "total_count");
        let done = v_u64(t, "transferred_count");
        let pct = if total > 0 { (done as f64 / total as f64) as f32 } else { 0.0 };
        let error = v_str(t, "error");
        let save_path = v_str(t, "save_path");
        let terminal = is_terminal(&status);

        // 左:分享文件类型图标
        let (icon, bg, fg) = design::file_visual(&title_text, p.is_dark);
        let tile = div()
            .size(px(44.))
            .rounded(px(12.))
            .bg(bg)
            .flex()
            .items_center()
            .justify_center()
            .flex_shrink_0()
            .child(Icon::new(icon).size(px(20.)).text_color(fg))
            .into_any_element();

        // 中:标题 / 标签 / 进度 / 元信息
        let mut title = h_flex().gap(px(8.)).items_center().min_w_0().overflow_hidden();
        title = title.child(
            div()
                .flex_1()
                .min_w_0()
                .truncate()
                .text_size(px(14.))
                .font_weight(gpui_kit::FontWeight::MEDIUM)
                .child(title_text),
        );
        title = title.child(status_tag(&status, p));
        if v_bool(t, "auto_download") {
            title = title.child(soft_pill("自动下载", p.selected_bg, p.muted));
        }
        if v_bool(t, "is_share_direct_download") {
            title = title.child(soft_pill("直下", p.selected_bg, p.muted));
        }

        let mut mid = v_flex().flex_1().min_w_0().gap(px(6.)).overflow_hidden().child(title);

        if !matches!(status.as_str(), "transferred" | "completed") {
            let fill = if is_active(&status) { p.primary } else { p.faint.alpha(0.5) };
            mid = mid.child(progress_bar(pct, fill, p.faint.alpha(0.18)));
        }

        let mut meta = h_flex()
            .gap(px(6.))
            .items_center()
            .min_w_0()
            .overflow_hidden()
            .text_size(px(12.))
            .text_color(p.faint);
        if matches!(status.as_str(), "transferred" | "completed") {
            meta = meta.child(format!("共 {total} 个文件"));
        } else {
            meta = meta.child(format!("{done} / {total} 文件"));
        }
        if !save_path.is_empty() {
            meta = meta.child(sep_dot(p));
            meta = meta.child(div().truncate().child(format!("保存至 {save_path}")));
        }
        if !error.is_empty() {
            meta = meta.child(sep_dot(p));
            meta = meta.child(div().truncate().text_color(p.danger).child(error));
        }
        mid = mid.child(meta);

        // 右:取消 / 删除
        let mut actions = h_flex().gap(px(2.)).items_center().flex_shrink_0();
        let call = |key: &str| gpui_kit::SharedString::from(format!("{key}:{id}"));
        if !terminal {
            let id_cancel = id.clone();
            actions = actions.child(circle_button(p, call("tf-cancel"), IconName::CircleStop, move |this, _, cx| {
                this.cancel(&id_cancel, cx)
            }, cx));
        }
        let id_del = id.clone();
        actions = actions.child(circle_button(p, call("tf-del"), IconName::Trash, move |this, _, cx| {
            this.delete(&id_del, cx)
        }, cx));

        task_card(p, tile, mid.into_any_element(), actions.into_any_element())
    }
}

impl Render for TransfersView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = design::palette(cx);
        let mut rows: Vec<Value> = cx.global::<TaskCache>().transfers.values().cloned().collect();
        rows.sort_by(|a, b| v_u64(b, "created_at").cmp(&v_u64(a, "created_at")));

        let active_count = rows.iter().filter(|t| is_active(&v_str(t, "status"))).count();
        let done_count = rows
            .iter()
            .filter(|t| matches!(v_str(t, "status").as_str(), "transferred" | "completed"))
            .count();

        let status_pill = if active_count > 0 {
            soft_pill(format!("{active_count} 个进行中"), p.primary_soft, p.primary)
        } else if done_count > 0 {
            soft_pill(format!("{done_count} 个已完成"), p.primary_soft, p.primary)
        } else {
            Tag::new().small().rounded_full().child("暂无任务")
        };

        let header = h_flex()
            .h(px(68.))
            .px(px(32.))
            .gap(px(12.))
            .items_center()
            .flex_shrink_0()
            .child(div().text_size(px(18.)).font_weight(gpui_kit::FontWeight::SEMIBOLD).child("转存"))
            .child(status_pill)
            .child(div().flex_1())
            .child(
                h_flex()
                    .gap(px(6.))
                    .items_center()
                    .child(
                        Button::new("tf-new")
                            .primary()
                            .small()
                            .icon(IconName::FolderInput)
                            .label("新建转存")
                            .on_click(cx.listener(|this, _, window, cx| this.open_transfer_dialog(false, window, cx))),
                    )
                    .child(
                        Button::new("tf-direct")
                            .ghost()
                            .small()
                            .icon(IconName::Download)
                            .label("分享直下")
                            .on_click(cx.listener(|this, _, window, cx| this.open_transfer_dialog(true, window, cx))),
                    )
                    .child(
                        Button::new("tf-refresh")
                            .ghost()
                            .small()
                            .icon(IconName::RefreshCw)
                            .on_click(cx.listener(|_, _, _, cx| {
                                cx.global::<Backend>().ws.request_snapshot();
                            })),
                    ),
            )
            .into_any_element();

        let body: AnyElement = if rows.is_empty() {
            empty_state(
                &p,
                IconName::FolderInput,
                "暂无转存任务",
                "粘贴分享链接即可把文件转存到网盘",
                Some(
                    Button::new("tf-empty-new")
                        .small()
                        .icon(IconName::FolderInput)
                        .label("新建转存")
                        .on_click(cx.listener(|this, _, window, cx| this.open_transfer_dialog(false, window, cx)))
                        .into_any_element(),
                ),
            )
        } else {
            let mut list = v_flex()
                .id("tf-scroll")
                .flex_1()
                .min_h_0()
                .gap(px(10.))
                .px(px(32.))
                .py(px(16.))
                .overflow_y_scrollbar();
            for t in &rows {
                list = list.child(self.render_row(t, &p, cx));
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
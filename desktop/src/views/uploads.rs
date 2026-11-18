//! 上传管理页(移植 UploadsView.vue):任务行实时进度,秒传/加密标签,
//! 暂停/恢复/重试/删除,批量操作。数据来自 TaskCache(WS 驱动)。

use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::progress::Progress;
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::tag::Tag;
use gpui_kit::component::{ActiveTheme as _, Icon, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AnyWindowHandle, App, AppContext as _, BorrowAppContext as _, Context, Entity, IntoElement, ParentElement as _,
    Render, Styled as _, Subscription, Window, div, px,
};
use serde_json::Value;

use crate::api::{self, download::BatchOperationRequest};
use crate::bridge::{EventHub, ToastKind, UiEvent};
use crate::components::toast;
use crate::state::task_cache::{v_bool, v_f64, v_str, v_u64};
use crate::state::{Backend, TaskCache};
use crate::util;
use crate::views::downloads::status_label;

fn is_active(status: &str) -> bool {
    matches!(status, "pending" | "uploading" | "encrypting" | "checking_rapid")
}

pub struct UploadsView {
    window_handle: AnyWindowHandle,
    _subscriptions: Vec<Subscription>,
}

impl UploadsView {
    pub fn new(hub: Entity<EventHub>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let subs = vec![cx.subscribe(&hub, |_, _, ev, cx| {
            if matches!(ev, UiEvent::WsMessage(_) | UiEvent::WsConnection(_) | UiEvent::AccountChanged { .. }) {
                cx.notify();
            }
        })];
        cx.global::<Backend>().ws.request_snapshot();
        Self { window_handle: window.window_handle(), _subscriptions: subs }
    }

    fn toast(&self, cx: &mut Context<Self>, kind: ToastKind, msg: impl Into<String>) {
        let msg = msg.into();
        let wh = self.window_handle;
        let _ = wh.update(cx, |_, w, cx| toast::show(w, cx, kind, msg.clone()));
    }

    fn rows(&self, cx: &App) -> Vec<Value> {
        let mut rows: Vec<Value> = cx.global::<TaskCache>().uploads.values().cloned().collect();
        rows.sort_by(|a, b| v_u64(b, "created_at").cmp(&v_u64(a, "created_at")));
        rows
    }

    fn pause_resume(&mut self, id: &str, status: &str, cx: &mut Context<Self>) {
        let b = cx.global::<Backend>();
        let (rt, api) = (b.rt.clone(), b.api.clone());
        let id = id.to_string();
        let pause = is_active(status);
        cx.spawn(async move |this, cx| {
            let (api2, id2) = (api.clone(), id.clone());
            let result = if pause {
                api::call_api(&rt, move || async move { api2.pause_upload(&id2).await }).await
            } else {
                api::call_api(&rt, move || async move { api2.resume_upload(&id2).await }).await
            };
            let _ = this.update(cx, |this, cx| {
                if let Err(e) = result { this.toast(cx, ToastKind::Error, e.message()); }
            });
        }).detach();
    }

    fn delete(&mut self, id: &str, cx: &mut Context<Self>) {
        let b = cx.global::<Backend>();
        let (rt, api) = (b.rt.clone(), b.api.clone());
        let id = id.to_string();
        cx.spawn(async move |this, cx| {
            let (api2, id2) = (api.clone(), id.clone());
            let result = api::call_api(&rt, move || async move { api2.delete_upload(&id2).await }).await;
            let id = id.clone();
            let _ = this.update(cx, |this, cx| {
                match result {
                    Ok(_) => {
                        cx.update_global::<TaskCache, _>(|c, _| { c.uploads.remove(&id); });
                        cx.global::<Backend>().ws.request_snapshot();
                        cx.notify();
                    }
                    Err(e) => this.toast(cx, ToastKind::Error, e.message()),
                }
            });
        }).detach();
    }

    fn batch_op(&mut self, op: &'static str, cx: &mut Context<Self>) {
        let b = cx.global::<Backend>();
        let (rt, api) = (b.rt.clone(), b.api.clone());
        cx.spawn(async move |this, cx| {
            let req = BatchOperationRequest { all: Some(true), ..Default::default() };
            let result: Result<(), api::ApiError> = match op {
                "pause" => { let (a, r) = (api.clone(), req.clone()); api::call_api(&rt, move || async move { a.batch_pause_uploads(r).await.map(|_| ()) }).await }
                "resume" => { let (a, r) = (api.clone(), req.clone()); api::call_api(&rt, move || async move { a.batch_resume_uploads(r).await.map(|_| ()) }).await }
                "clear_completed" => { let a = api.clone(); api::call_api(&rt, move || async move { a.clear_completed_uploads().await.map(|_| ()) }).await }
                _ => { let a = api.clone(); api::call_api(&rt, move || async move { a.clear_failed_uploads().await.map(|_| ()) }).await }
            };
            let _ = this.update(cx, |this, cx| {
                match result {
                    Ok(_) => {
                        cx.update_global::<TaskCache, _>(|c, _| match op {
                            "clear_completed" => { c.uploads.retain(|_, v| v_str(v, "status") != "completed"); }
                            "clear_failed" => { c.uploads.retain(|_, v| v_str(v, "status") != "failed"); }
                            _ => {}
                        });
                        cx.global::<Backend>().ws.request_snapshot();
                        this.toast(cx, ToastKind::Success, "操作完成");
                        cx.notify();
                    }
                    Err(e) => this.toast(cx, ToastKind::Error, e.message()),
                }
            });
        }).detach();
    }

    fn render_row(&self, task: &Value, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let id = v_str(task, "id");
        let status = v_str(task, "status");
        let name = crate::api::file::basename(&v_str(task, "local_path"));
        let total = v_u64(task, "total_size");
        let done = v_u64(task, "uploaded_size");
        let speed = v_u64(task, "speed");
        let progress = if total > 0 { (done as f64 / total as f64 * 100.0) as f32 } else { 0.0 };
        let error = v_str(task, "error");
        let active = is_active(&status);
        let terminal = matches!(status.as_str(), "completed" | "failed" | "rapid_upload_success" | "cancelled");
        let encrypting = status == "encrypting";
        let encrypt_progress = v_f64(task, "encrypt_progress");

        let id_pr = id.clone();
        let status_pr = status.clone();
        let id_del = id.clone();
        let id_retry = id.clone();

        let mut row = v_flex().gap_1p5().px_3().py_2().rounded_lg().bg(theme.secondary);
        let mut top = h_flex().gap_2().items_center();
        top = top.child(Icon::new(IconName::FileText).size(px(18.)).text_color(theme.primary));
        top = top.child(div().flex_1().truncate().child(name));
        top = top.child(Tag::new().small().child(status_label(&status)));
        if v_bool(task, "is_rapid_upload") || status == "rapid_upload_success" {
            top = top.child(Tag::new().small().child("秒传"));
        }
        if v_bool(task, "encrypt_enabled") {
            top = top.child(Tag::new().small().child("🔒"));
        }
        if !terminal {
            top = top.child(Button::new(format!("ul-pr-{id}")).ghost().small()
                .icon(if active { IconName::Pause } else { IconName::Play })
                .on_click(cx.listener(move |this, _, _, cx| this.pause_resume(&id_pr, &status_pr, cx))));
        } else if status == "failed" {
            top = top.child(Button::new(format!("ul-retry-{id_retry}")).ghost().small().icon(IconName::RotateCw)
                .on_click(cx.listener(move |this, _, _, cx| this.pause_resume(&id_retry, "paused", cx))));
        }
        top = top.child(Button::new(format!("ul-del-{id_del}")).ghost().small().icon(IconName::Trash)
            .on_click(cx.listener(move |this, _, _, cx| this.delete(&id_del, cx))));
        row = row.child(top);
        row = row.child(div().text_color(theme.muted_foreground).truncate().child(format!("{} → {}", v_str(task, "local_path"), v_str(task, "remote_path"))));
        row = row.child(Progress::new(format!("ul-prog-{id}")).value(progress));
        row = row.child(div().text_color(theme.muted_foreground).child(format!("{} / {} · {}", util::format_size(done), util::format_size(total), util::format_speed(speed))));
        if encrypting {
            row = row.child(Progress::new(format!("ul-enc-{id}")).value((encrypt_progress * 100.0) as f32));
        }
        if !error.is_empty() {
            row = row.child(div().text_color(theme.danger).child(error));
        }
        row
    }
}

impl Render for UploadsView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let rows = self.rows(cx);
        let active_count = rows.iter().filter(|t| is_active(&v_str(t, "status"))).count();

        let toolbar = h_flex().gap_2().items_center().px_4().py_1p5()
            .child(div().flex_1().text_color(theme.muted_foreground).child(format!("{active_count} 个活跃任务")))
            .child(Button::new("ul-pause-all").small().label("全部暂停").on_click(cx.listener(|this, _, _, cx| this.batch_op("pause", cx))))
            .child(Button::new("ul-resume-all").small().label("全部继续").on_click(cx.listener(|this, _, _, cx| this.batch_op("resume", cx))))
            .child(Button::new("ul-clear-done").small().label("清除已完成").on_click(cx.listener(|this, _, _, cx| this.batch_op("clear_completed", cx))))
            .child(Button::new("ul-clear-fail").small().label("清除失败").on_click(cx.listener(|this, _, _, cx| this.batch_op("clear_failed", cx))))
            .child(Button::new("ul-refresh").small().icon(IconName::RefreshCw).on_click(cx.listener(|_, _, _, cx| { cx.global::<Backend>().ws.request_snapshot(); })));

        let mut list = v_flex().gap_2().p_3().overflow_y_scrollbar();
        if rows.is_empty() {
            list = list.child(div().py_12().rounded_lg().bg(theme.secondary).text_center().text_color(theme.muted_foreground).child("暂无上传任务"));
        }
        for t in &rows {
            list = list.child(self.render_row(t, cx));
        }

        v_flex().flex_1().overflow_hidden()
            .child(toolbar)
            .child(div().flex_1().child(list))
    }
}
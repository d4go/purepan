//! 上传管理页:总览头部(活跃数 / 实时总速度) + 三段式任务卡片(文件类型图标、
//! 进度、元信息、操作),秒传/加密标签,批量操作。数据来自 TaskCache(WS 驱动)。

use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::tag::Tag;
use gpui_kit::component::{Disableable as _, Icon, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AnyElement, AnyWindowHandle, BorrowAppContext as _, Context, Entity,
    InteractiveElement as _, IntoElement, ParentElement as _, Render, Styled as _, Subscription,
    Window, div, px,
};
use serde_json::Value;

use crate::api::{self, download::BatchOperationRequest};
use crate::bridge::{EventHub, ToastKind, UiEvent};
use crate::components::app_sidebar::Nav;
use crate::components::task_ui::{
    circle_button, empty_state, progress_bar, sep_dot, soft_pill, status_tag, task_card,
};
use crate::components::toast;
use crate::design::{self, Palette};
use crate::state::task_cache::{v_bool, v_f64, v_str, v_u64};
use crate::state::{Backend, TaskCache};
use crate::util;

fn is_active(status: &str) -> bool {
    matches!(status, "pending" | "uploading" | "encrypting" | "checking_rapid")
}

pub struct UploadsView {
    hub: Entity<EventHub>,
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
        Self { hub, window_handle: window.window_handle(), _subscriptions: subs }
    }

    fn toast(&self, cx: &mut Context<Self>, kind: ToastKind, msg: impl Into<String>) {
        let msg = msg.into();
        let wh = self.window_handle;
        let _ = wh.update(cx, |_, w, cx| toast::show(w, cx, kind, msg.clone()));
    }

    fn rows(&self, cx: &Context<Self>) -> Vec<Value> {
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

    fn render_row(&self, task: &Value, p: &Palette, cx: &mut Context<Self>) -> AnyElement {
        let id = v_str(task, "id");
        let status = v_str(task, "status");
        let local_path = v_str(task, "local_path");
        let remote_path = v_str(task, "remote_path");
        let name = crate::api::file::basename(&local_path);
        let total = v_u64(task, "total_size");
        let done = v_u64(task, "uploaded_size");
        let speed = v_u64(task, "speed");
        let error = v_str(task, "error");
        let active = is_active(&status);
        let paused = status == "paused";
        let completed = matches!(status.as_str(), "completed" | "rapid_upload_success");
        let failed = status == "failed";
        let encrypting = status == "encrypting";
        let pct = if total > 0 { (done as f64 / total as f64) as f32 } else { 0.0 };
        let bar_pct = if encrypting {
            (v_f64(task, "encrypt_progress") as f32).clamp(0.0, 1.0)
        } else {
            pct
        };

        // 左:文件类型图标
        let (icon, bg, fg) = design::file_visual(&name, p.is_dark);
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

        // 中:名称 / 进度 / 元信息
        let mut title = h_flex().gap(px(8.)).items_center().min_w_0().overflow_hidden();
        title = title.child(
            div()
                .flex_1()
                .min_w_0()
                .truncate()
                .text_size(px(14.))
                .font_weight(gpui_kit::FontWeight::MEDIUM)
                .child(name),
        );
        if v_bool(task, "encrypt_enabled") {
            title = title.child(div().text_size(px(11.)).child("🔒"));
        }
        title = title.child(status_tag(&status, p));
        if v_bool(task, "is_rapid_upload") || status == "rapid_upload_success" {
            title = title.child(soft_pill("秒传", p.selected_bg, p.muted));
        }

        let mut mid = v_flex().flex_1().min_w_0().gap(px(6.)).overflow_hidden().child(title);

        if !completed {
            let fill = if active { p.primary } else { p.faint.alpha(0.5) };
            mid = mid.child(progress_bar(bar_pct, fill, p.faint.alpha(0.18)));
        }

        let mut meta = h_flex()
            .gap(px(6.))
            .items_center()
            .min_w_0()
            .overflow_hidden()
            .text_size(px(12.))
            .text_color(p.faint);
        if completed {
            meta = meta.child(format!("共 {}", util::format_size(total)));
        } else {
            meta = meta.child(format!("{} / {}", util::format_size(done), util::format_size(total)));
            if active && speed > 0 {
                meta = meta.child(sep_dot(p));
                meta = meta.child(
                    div()
                        .text_color(p.primary)
                        .font_weight(gpui_kit::FontWeight::MEDIUM)
                        .child(format!("↑ {}", util::format_speed(speed))),
                );
                let remaining = total.saturating_sub(done) / speed.max(1);
                meta = meta.child(sep_dot(p));
                meta = meta.child(format!("剩余 {}", util::format_eta(Some(remaining))));
            }
        }
        if failed && !error.is_empty() {
            meta = meta.child(sep_dot(p));
            meta = meta.child(div().truncate().text_color(p.danger).child(error));
        } else if !remote_path.is_empty() {
            meta = meta.child(sep_dot(p));
            meta = meta.child(div().truncate().child(format!("→ {remote_path}")));
        }
        mid = mid.child(meta);

        // 右:操作区
        let mut actions = h_flex().gap(px(2.)).items_center().flex_shrink_0();
        let call = |key: &str| gpui_kit::SharedString::from(format!("{key}:{id}"));

        if active || paused {
            let icon = if paused { IconName::Play } else { IconName::Pause };
            let id_p = id.clone();
            let status_p = status.clone();
            actions = actions.child(circle_button(p, call("ul-pr"), icon, move |this, _, cx| {
                this.pause_resume(&id_p, &status_p, cx)
            }, cx));
        } else if failed {
            let id_r = id.clone();
            actions = actions.child(circle_button(p, call("ul-retry"), IconName::RotateCw, move |this, _, cx| {
                this.pause_resume(&id_r, "paused", cx)
            }, cx));
        }
        let id_del = id.clone();
        actions = actions.child(circle_button(p, call("ul-del"), IconName::Trash, move |this, _window, cx| {
            this.delete(&id_del, cx)
        }, cx));

        task_card(p, tile, mid.into_any_element(), actions.into_any_element())
    }
}

impl Render for UploadsView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = design::palette(cx);
        let rows = self.rows(cx);

        let (active_count, paused_count, completed_count, failed_count, total_speed) = {
            let mut active = 0usize;
            let mut paused = 0usize;
            let mut completed = 0usize;
            let mut failed = 0usize;
            for t in &rows {
                match v_str(t, "status").as_str() {
                    "paused" => paused += 1,
                    "completed" | "rapid_upload_success" => completed += 1,
                    "failed" => failed += 1,
                    s if is_active(s) => active += 1,
                    _ => {}
                }
            }
            let speed: u64 = rows
                .iter()
                .filter(|t| is_active(&v_str(t, "status")))
                .map(|t| v_u64(t, "speed"))
                .sum();
            (active, paused, completed, failed, speed)
        };

        let status_pill = if active_count > 0 {
            soft_pill(format!("{active_count} 个活跃任务"), p.primary_soft, p.primary)
        } else {
            Tag::new().small().rounded_full().child("无活跃任务")
        };

        let speed_chip = (total_speed > 0).then(|| {
            h_flex()
                .gap(px(4.))
                .items_center()
                .text_size(px(13.))
                .font_weight(gpui_kit::FontWeight::MEDIUM)
                .text_color(p.primary)
                .child("↑")
                .child(util::format_speed(total_speed))
        });

        let header = h_flex()
            .h(px(68.))
            .px(px(32.))
            .gap(px(12.))
            .items_center()
            .flex_shrink_0()
            .child(div().text_size(px(18.)).font_weight(gpui_kit::FontWeight::SEMIBOLD).child("上传"))
            .child(status_pill)
            .when_some(speed_chip, |this, chip| this.child(chip))
            .child(div().flex_1())
            .child(
                h_flex()
                    .gap(px(6.))
                    .items_center()
                    .child(
                        Button::new("ul-pause-all")
                            .ghost()
                            .small()
                            .icon(IconName::Pause)
                            .label("全部暂停")
                            .disabled(active_count == 0)
                            .on_click(cx.listener(|this, _, _, cx| this.batch_op("pause", cx))),
                    )
                    .child(
                        Button::new("ul-resume-all")
                            .ghost()
                            .small()
                            .icon(IconName::Play)
                            .label("全部继续")
                            .disabled(paused_count == 0)
                            .on_click(cx.listener(|this, _, _, cx| this.batch_op("resume", cx))),
                    )
                    .child(
                        Button::new("ul-clear-done")
                            .ghost()
                            .small()
                            .icon(IconName::CircleCheck)
                            .label("清除已完成")
                            .disabled(completed_count == 0)
                            .on_click(cx.listener(|this, _, _, cx| this.batch_op("clear_completed", cx))),
                    )
                    .child(
                        Button::new("ul-clear-fail")
                            .ghost()
                            .small()
                            .icon(IconName::Trash)
                            .label("清除失败")
                            .disabled(failed_count == 0)
                            .on_click(cx.listener(|this, _, _, cx| this.batch_op("clear_failed", cx))),
                    )
                    .child(
                        Button::new("ul-refresh")
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
            let hub = self.hub.clone();
            empty_state(
                &p,
                IconName::Upload,
                "暂无上传任务",
                "在文件页上传文件后,进度会实时显示在这里",
                Some(
                    Button::new("ul-goto-files")
                        .small()
                        .icon(IconName::Folder)
                        .label("去上传")
                        .on_click(cx.listener(move |_, _, _, cx| {
                            hub.update(cx, |_, cx| cx.emit(UiEvent::Navigate(Nav::Files)));
                        }))
                        .into_any_element(),
                ),
            )
        } else {
            let mut list = v_flex()
                .id("ul-scroll")
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
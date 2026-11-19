//! 下载管理页:总览头部(活跃数 / 实时总速度) + 三段式任务卡片(类型图标、进度、
//! 元信息、操作),文件夹聚合行 + 单文件行,实时进度来自 TaskCache(WS 驱动)。

use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::dialog::Dialog;
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::tag::Tag;
use gpui_kit::component::{Disableable as _, Icon, Sizable as _, WindowExt as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AnyElement, AnyWindowHandle, AppContext as _, BorrowAppContext as _, Context, Entity,
    InteractiveElement as _, IntoElement, ParentElement as _, Render, Styled as _, Subscription,
    Window, div, px, rgb,
};
use serde_json::Value;

use crate::api::{self, download::BatchOperationRequest};
use crate::bridge::{EventHub, ToastKind, UiEvent};
use crate::components::app_sidebar::Nav;
use crate::components::task_ui::{
    circle_button, progress_bar, sep_dot, soft_pill, status_label, status_tag, task_card,
};
use crate::components::toast;
use crate::design::{self, Palette};
use crate::state::task_cache::{v_bool, v_f64, v_str, v_u64};
use crate::state::{Backend, TaskCache};
use crate::util;

fn is_active(status: &str) -> bool {
    matches!(status, "pending" | "downloading" | "uploading" | "decrypting" | "encrypting" | "scanning" | "checking_rapid")
}

pub struct DownloadsView {
    hub: Entity<EventHub>,
    window_handle: AnyWindowHandle,
    delete_target: Option<(String, bool)>, // (id, is_folder)
    _subscriptions: Vec<Subscription>,
}

impl DownloadsView {
    pub fn new(hub: Entity<EventHub>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let subs = vec![cx.subscribe(&hub, |_, _, ev, cx| {
            if matches!(ev, UiEvent::WsMessage(_) | UiEvent::WsConnection(_) | UiEvent::AccountChanged { .. }) {
                cx.notify();
            }
        })];
        // 进入页面请求快照
        cx.global::<Backend>().ws.request_snapshot();
        Self { hub, window_handle: window.window_handle(), delete_target: None, _subscriptions: subs }
    }

    fn toast(&self, cx: &mut Context<Self>, kind: ToastKind, msg: impl Into<String>) {
        let msg = msg.into();
        let wh = self.window_handle;
        let _ = wh.update(cx, |_, w, cx| toast::show(w, cx, kind, msg.clone()));
    }

    fn rows(&self, cx: &Context<Self>) -> (Vec<Value>, Vec<Value>) {
        let cache = cx.global::<TaskCache>();
        let mut folders: Vec<Value> = cache.folders.values().cloned().collect();
        folders.sort_by(|a, b| v_u64(b, "created_at").cmp(&v_u64(a, "created_at")));
        // 单文件:排除属于文件夹组的(group_id 非空)
        let mut files: Vec<Value> = cache.downloads.values()
            .filter(|v| v.get("group_id").and_then(|g| g.as_str()).unwrap_or("").is_empty())
            .cloned().collect();
        files.sort_by(|a, b| v_u64(b, "created_at").cmp(&v_u64(a, "created_at")));
        (folders, files)
    }

    fn pause_resume(&mut self, id: &str, status: &str, is_folder: bool, cx: &mut Context<Self>) {
        let b = cx.global::<Backend>();
        let (rt, api) = (b.rt.clone(), b.api.clone());
        let id = id.to_string();
        let pause = is_active(status);
        cx.spawn(async move |this, cx| {
            let (api2, id2) = (api.clone(), id.clone());
            let result = if pause {
                api::call_api(&rt, move || async move {
                    if is_folder { api2.pause_folder_download(&id2).await } else { api2.pause_download(&id2).await }
                }).await
            } else {
                api::call_api(&rt, move || async move {
                    if is_folder { api2.resume_folder_download(&id2).await } else { api2.resume_download(&id2).await }
                }).await
            };
            let _ = this.update(cx, |this, cx| {
                if let Err(e) = result { this.toast(cx, ToastKind::Error, e.message()); }
            });
        }).detach();
    }

    fn open_delete(&mut self, id: &str, is_folder: bool, window: &mut Window, cx: &mut Context<Self>) {
        self.delete_target = Some((id.to_string(), is_folder));
        let view = cx.entity().clone();
        let v2 = view.clone();
        let v3 = view.clone();
        window.open_dialog(cx, move |dialog: Dialog, _, _| {
            let v4 = v2.clone();
            let v_files = v3.clone();
            dialog.title("删除下载任务").width(px(460.))
                .child("请选择删除方式:")
                .footer(h_flex().gap_2().justify_end()
                    .child(Button::new("dl-cancel").label("取消").on_click(|_, w, cx| w.close_dialog(cx)))
                    .child(Button::new("dl-task").label("仅删除任务").on_click(move |_, _, cx| {
                        v4.update(cx, |v, cx| v.confirm_delete(false, cx));
                    }))
                    .child(Button::new("dl-files").danger().label("删除任务和本地文件").on_click(move |_, _, cx| {
                        v_files.update(cx, |v, cx| v.confirm_delete(true, cx));
                    })))
                .on_ok({ let v = v2.clone(); move |_, _, cx| { v.update(cx, |v, cx| v.confirm_delete(false, cx)); true } })
        });
    }

    fn confirm_delete(&mut self, delete_files: bool, cx: &mut Context<Self>) {
        let Some((id, is_folder)) = self.delete_target.clone() else { return };
        let b = cx.global::<Backend>();
        let (rt, api) = (b.rt.clone(), b.api.clone());
        cx.spawn(async move |this, cx| {
            let result = if is_folder {
                // 文件夹任务由独立的 folder_manager 管理
                let (api2, id2) = (api.clone(), id.clone());
                api::call_api(&rt, move || async move { api2.cancel_folder_download(&id2, delete_files).await }).await
            } else {
                let (api2, id2) = (api.clone(), id.clone());
                api::call_api(&rt, move || async move { api2.delete_download(&id2, delete_files).await }).await
            };
            let id = id.clone();
            let _ = this.update(cx, |this, cx| {
                let _ = this.window_handle.update(cx, |_, w, cx| w.close_all_dialogs(cx));
                match result {
                    Ok(_) => {
                        // 不依赖 WS 事件:立即从缓存移除并请服务器快照兜底
                        cx.update_global::<TaskCache, _>(|c, _| {
                            if is_folder {
                                c.folders.remove(&id);
                                c.downloads.retain(|_, v| v_str(v, "group_id") != id);
                            } else {
                                c.downloads.remove(&id);
                            }
                        });
                        cx.global::<Backend>().ws.request_snapshot();
                        this.toast(cx, ToastKind::Success, if delete_files { "任务和本地文件已删除" } else { "任务已删除" });
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
                "pause" => { let (a, r) = (api.clone(), req.clone()); api::call_api(&rt, move || async move { a.batch_pause_downloads(r).await.map(|_| ()) }).await }
                "resume" => { let (a, r) = (api.clone(), req.clone()); api::call_api(&rt, move || async move { a.batch_resume_downloads(r).await.map(|_| ()) }).await }
                "clear_completed" => { let a = api.clone(); api::call_api(&rt, move || async move { a.clear_completed_downloads().await.map(|_| ()) }).await }
                _ => { let a = api.clone(); api::call_api(&rt, move || async move { a.clear_failed_downloads().await.map(|_| ()) }).await }
            };
            let _ = this.update(cx, |this, cx| {
                match result {
                    Ok(_) => {
                        // 清除类操作后端不逐条发 deleted 事件,本地立即合并 + 快照兜底
                        cx.update_global::<TaskCache, _>(|c, _| match op {
                            "clear_completed" => {
                                c.downloads.retain(|_, v| v_str(v, "status") != "completed");
                                c.folders.retain(|_, v| v_str(v, "status") != "completed");
                            }
                            "clear_failed" => {
                                c.downloads.retain(|_, v| v_str(v, "status") != "failed");
                                c.folders.retain(|_, v| v_str(v, "status") != "failed");
                            }
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

    fn open_folder_detail(&mut self, folder_id: &str, window: &mut Window, cx: &mut Context<Self>) {
        let folder_id = folder_id.to_string();
        let hub = self.hub.clone();
        let detail = cx.new(|cx| FolderDetail::new(folder_id.clone(), hub.clone(), cx));
        let title = cx.global::<TaskCache>().folders.get(&folder_id)
            .map(|f| v_str(f, "name")).unwrap_or_else(|| "文件夹详情".into());
        window.open_dialog(cx, move |d: Dialog, _, _| d.title(title.clone()).width(px(640.)).child(detail.clone()));
    }

    /// 打开下载文件所在目录(不是文件本身),失败时给出提示而非静默。
    fn open_in_folder(&mut self, local_path: &str, cx: &mut Context<Self>) {
        let path = std::path::PathBuf::from(local_path);
        let dir = if path.is_dir() {
            path
        } else {
            match path.parent() {
                Some(p) if !p.as_os_str().is_empty() => p.to_path_buf(),
                _ => {
                    self.toast(cx, ToastKind::Warning, "无法确定文件所在目录");
                    return;
                }
            }
        };
        if !dir.exists() {
            self.toast(cx, ToastKind::Error, format!("目录不存在: {}", dir.display()));
            return;
        }
        if let Err(e) = open::that(&dir) {
            self.toast(cx, ToastKind::Error, format!("打开文件夹失败: {e}"));
        }
    }

    fn render_row(&self, task: &Value, is_folder: bool, p: &Palette, cx: &mut Context<Self>) -> AnyElement {
        let id = v_str(task, "id");
        let status = v_str(task, "status");
        let name = if is_folder { v_str(task, "name") } else { crate::api::file::basename(&v_str(task, "remote_path")) };
        let total = v_u64(task, "total_size");
        let done = v_u64(task, "downloaded_size");
        let speed = if is_folder {
            // 文件夹自身没有速度字段,汇总子任务速度
            cx.global::<TaskCache>().downloads.values()
                .filter(|v| v_str(v, "group_id") == id)
                .map(|v| v_u64(v, "speed"))
                .sum()
        } else {
            v_u64(task, "speed")
        };
        let error = v_str(task, "error");
        let local_path = if is_folder { v_str(task, "local_root") } else { v_str(task, "local_path") };
        let active = is_active(&status);
        let paused = status == "paused";
        let completed = status == "completed";
        let failed = status == "failed";
        let decrypting = status == "decrypting";
        let pct = if total > 0 { (done as f64 / total as f64) as f32 } else { 0.0 };
        let bar_pct = if decrypting {
            (v_f64(task, "decrypt_progress") as f32).clamp(0.0, 1.0)
        } else {
            pct
        };

        // ── 左:文件类型图标 ──
        let tile: AnyElement = if is_folder {
            div()
                .size(px(44.))
                .rounded(px(12.))
                .bg(p.folder_tile(false))
                .flex()
                .items_center()
                .justify_center()
                .flex_shrink_0()
                .child(Icon::new(IconName::Folder).size(px(22.)).text_color(rgb(0xFFFFFF).alpha(0.92)))
                .into_any_element()
        } else {
            let (icon, bg, fg) = design::file_visual(&name, p.is_dark);
            div()
                .size(px(44.))
                .rounded(px(12.))
                .bg(bg)
                .flex()
                .items_center()
                .justify_center()
                .flex_shrink_0()
                .child(Icon::new(icon).size(px(20.)).text_color(fg))
                .into_any_element()
        };

        // ── 中:名称 / 进度条 / 元信息 ──
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
        if v_bool(task, "is_encrypted") {
            title = title.child(div().text_size(px(11.)).child("🔒"));
        }
        title = title.child(status_tag(&status, p));

        let mut mid = v_flex().flex_1().min_w_0().gap(px(6.)).overflow_hidden().child(title);

        if !completed {
            let fill = if active { p.primary } else { p.faint.alpha(0.5) };
            let track = p.faint.alpha(0.18);
            mid = mid.child(progress_bar(bar_pct, fill, track));
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
            if is_folder {
                meta = meta.child(format!(
                    "{}/{} 文件",
                    v_u64(task, "completed_count"),
                    v_u64(task, "total_files"),
                ));
                meta = meta.child(sep_dot(p));
            }
            meta = meta.child(format!("{} / {}", util::format_size(done), util::format_size(total)));
            if active && speed > 0 {
                meta = meta.child(sep_dot(p));
                meta = meta.child(
                    div()
                        .text_color(p.primary)
                        .font_weight(gpui_kit::FontWeight::MEDIUM)
                        .child(format!("↓ {}", util::format_speed(speed))),
                );
                let remaining = total.saturating_sub(done) / speed.max(1);
                meta = meta.child(sep_dot(p));
                meta = meta.child(format!("剩余 {}", util::format_eta(Some(remaining))));
            }
        }
        if failed && !error.is_empty() {
            meta = meta.child(sep_dot(p));
            meta = meta.child(div().truncate().text_color(p.danger).child(error));
        }
        mid = mid.child(meta);

        // ── 右:操作区 ──
        let mut actions = h_flex().gap(px(2.)).items_center().flex_shrink_0();
        let can_open = !local_path.is_empty();
        let call = |key: &str| gpui_kit::SharedString::from(format!("{key}:{id}"));

        if active || paused {
            let icon = if paused { IconName::Play } else { IconName::Pause };
            let id_p = id.clone();
            let status_p = status.clone();
            actions = actions.child(circle_button(p, call("dl-pr"), icon, move |this, _, cx| {
                this.pause_resume(&id_p, &status_p, is_folder, cx)
            }, cx));
        } else if failed {
            let id_r = id.clone();
            actions = actions.child(circle_button(p, call("dl-retry"), IconName::RotateCw, move |this, _, cx| {
                this.pause_resume(&id_r, "paused", is_folder, cx)
            }, cx));
        }

        if is_folder {
            let id_detail = id.clone();
            actions = actions.child(circle_button(p, call("dl-detail"), IconName::ListTree, move |this, window, cx| {
                this.open_folder_detail(&id_detail, window, cx)
            }, cx));
        }

        if can_open && completed {
            // 已完成:把「打开文件夹」提权为带文字的次级按钮
            let open_path = local_path.clone();
            actions = actions.child(
                Button::new(call("dl-open-folder"))
                    .secondary()
                    .small()
                    .icon(IconName::FolderOpen)
                    .label("打开文件夹")
                    .on_click(cx.listener(move |this, _, _, cx| this.open_in_folder(&open_path, cx))),
            );
        } else if can_open {
            let open_path = local_path.clone();
            actions = actions.child(circle_button(p, call("dl-open"), IconName::FolderOpen, move |this, _, cx| {
                this.open_in_folder(&open_path, cx)
            }, cx));
        }

        let id_del = id.clone();
        actions = actions.child(circle_button(p, call("dl-del"), IconName::Trash, move |this, window, cx| {
            this.open_delete(&id_del, is_folder, window, cx)
        }, cx));

        task_card(p, tile, mid.into_any_element(), actions.into_any_element())
    }
}

impl Render for DownloadsView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = design::palette(cx);
        let (folders, files) = self.rows(cx);

        let (active_count, paused_count, completed_count, failed_count, total_speed) = {
            let all = folders.iter().chain(files.iter());
            let mut active = 0usize;
            let mut paused = 0usize;
            let mut completed = 0usize;
            let mut failed = 0usize;
            for t in all {
                match v_str(t, "status").as_str() {
                    "paused" => paused += 1,
                    "completed" => completed += 1,
                    "failed" => failed += 1,
                    s if is_active(s) => active += 1,
                    _ => {}
                }
            }
            let speed: u64 = cx.global::<TaskCache>().downloads.values()
                .filter(|v| is_active(&v_str(v, "status")))
                .map(|v| v_u64(v, "speed"))
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
                .child("↓")
                .child(util::format_speed(total_speed))
        });

        let header = h_flex()
            .h(px(68.))
            .px(px(32.))
            .gap(px(12.))
            .items_center()
            .flex_shrink_0()
            .child(div().text_size(px(18.)).font_weight(gpui_kit::FontWeight::SEMIBOLD).child("下载"))
            .child(status_pill)
            .when_some(speed_chip, |this, chip| this.child(chip))
            .child(div().flex_1())
            .child(
                h_flex()
                    .gap(px(6.))
                    .items_center()
                    .child(
                        Button::new("dl-pause-all")
                            .ghost()
                            .small()
                            .icon(IconName::Pause)
                            .label("全部暂停")
                            .disabled(active_count == 0)
                            .on_click(cx.listener(|this, _, _, cx| this.batch_op("pause", cx))),
                    )
                    .child(
                        Button::new("dl-resume-all")
                            .ghost()
                            .small()
                            .icon(IconName::Play)
                            .label("全部继续")
                            .disabled(paused_count == 0)
                            .on_click(cx.listener(|this, _, _, cx| this.batch_op("resume", cx))),
                    )
                    .child(
                        Button::new("dl-clear-done")
                            .ghost()
                            .small()
                            .icon(IconName::CircleCheck)
                            .label("清除已完成")
                            .disabled(completed_count == 0)
                            .on_click(cx.listener(|this, _, _, cx| this.batch_op("clear_completed", cx))),
                    )
                    .child(
                        Button::new("dl-clear-fail")
                            .ghost()
                            .small()
                            .icon(IconName::Trash)
                            .label("清除失败")
                            .disabled(failed_count == 0)
                            .on_click(cx.listener(|this, _, _, cx| this.batch_op("clear_failed", cx))),
                    )
                    .child(
                        Button::new("dl-refresh")
                            .ghost()
                            .small()
                            .icon(IconName::RefreshCw)
                            .on_click(cx.listener(|_, _, _, cx| {
                                cx.global::<Backend>().ws.request_snapshot();
                            })),
                    ),
            )
            .into_any_element();

        let body: AnyElement = if folders.is_empty() && files.is_empty() {
            let hub = self.hub.clone();
            crate::components::task_ui::empty_state(
                &p,
                IconName::Download,
                "暂无下载任务",
                "在文件页选择文件下载后,进度会实时显示在这里",
                Some(
                    Button::new("dl-goto-files")
                        .small()
                        .icon(IconName::Folder)
                        .label("去下载文件")
                        .on_click(cx.listener(move |_, _, _, cx| {
                            hub.update(cx, |_, cx| cx.emit(UiEvent::Navigate(Nav::Files)));
                        }))
                        .into_any_element(),
                ),
            )
        } else {
            let mut list = v_flex()
                .id("dl-scroll")
                .flex_1()
                .min_h_0()
                .gap(px(10.))
                .px(px(32.))
                .py(px(16.))
                .overflow_y_scrollbar();
            for f in &folders {
                list = list.child(self.render_row(f, true, &p, cx));
            }
            for f in &files {
                list = list.child(self.render_row(f, false, &p, cx));
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

// ───────────────────────── 文件夹详情 ─────────────────────────

struct FolderDetail {
    folder_id: String,
    ws: crate::ws::WsHandle,
    _sub: Subscription,
}

impl FolderDetail {
    fn new(folder_id: String, hub: Entity<EventHub>, cx: &mut Context<Self>) -> Self {
        let ws = cx.global::<Backend>().ws.clone();
        // 子任务事件带 group_id,后端只推给订阅了 `download:<gid>` 的连接
        ws.subscribe(vec![format!("download:{folder_id}")]);
        let sub = cx.subscribe(&hub, |_, _, ev, cx| {
            if matches!(ev, UiEvent::WsMessage(_) | UiEvent::AccountChanged { .. }) {
                cx.notify();
            }
        });
        Self { folder_id, ws, _sub: sub }
    }
}

impl Drop for FolderDetail {
    fn drop(&mut self) {
        self.ws.unsubscribe(vec![format!("download:{}", self.folder_id)]);
    }
}

impl Render for FolderDetail {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = design::palette(cx);
        let cache = cx.global::<TaskCache>();
        let folder = cache.folders.get(&self.folder_id).cloned().unwrap_or(Value::Null);
        let mut children: Vec<Value> = cache.downloads.values()
            .filter(|v| v_str(v, "group_id") == self.folder_id)
            .cloned().collect();
        children.sort_by(|a, b| v_str(a, "relative_path").cmp(&v_str(b, "relative_path")));

        let stats = format!(
            "{}/{} 文件 · {} / {} · {}",
            v_u64(&folder, "completed_count"), v_u64(&folder, "total_files"),
            util::format_size(v_u64(&folder, "downloaded_size")), util::format_size(v_u64(&folder, "total_size")),
            status_label(&v_str(&folder, "status")),
        );

        let mut list = v_flex().max_h(px(400.)).overflow_y_scrollbar();
        if children.is_empty() {
            list = list.child(div().py_4().text_center().text_color(p.muted).child("暂无子任务"));
        }
        for c in &children {
            let total = v_u64(c, "total_size");
            let done = v_u64(c, "downloaded_size");
            let pct = if total > 0 { (done as f64 / total as f64) as f32 } else { 0.0 };
            let name = {
                let rel = v_str(c, "relative_path");
                if rel.is_empty() { crate::api::file::basename(&v_str(c, "remote_path")) } else { rel }
            };
            list = list.child(
                v_flex().gap(px(6.)).px(px(12.)).py(px(8.)).rounded(px(10.)).bg(p.sidebar)
                    .child(
                        h_flex().gap(px(8.)).items_center()
                            .child(div().flex_1().min_w_0().truncate().child(name))
                            .child(status_tag(&v_str(c, "status"), &p))
                            .child(div().text_size(px(12.)).text_color(p.faint).child(util::format_size(total))),
                    )
                    .child(progress_bar(pct, p.primary, p.faint.alpha(0.18))),
            );
        }

        v_flex()
            .gap(px(12.))
            .child(div().text_size(px(12.)).text_color(p.muted).child(stats))
            .child(list)
    }
}

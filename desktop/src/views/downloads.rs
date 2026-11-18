//! 下载管理页(移植 DownloadsView.vue):文件夹聚合行 + 单文件行,实时进度,
//! 暂停/恢复/重试/删除(三向)/文件夹详情,批量操作。数据来自 TaskCache(WS 驱动)。

use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::dialog::Dialog;
use gpui_kit::component::progress::Progress;
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::tag::Tag;
use gpui_kit::component::{ActiveTheme as _, Disableable as _, Icon, Sizable as _, WindowExt as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AnyWindowHandle, App, AppContext as _, BorrowAppContext as _, Context, Entity, IntoElement,
    ParentElement as _, Render, Styled as _, Subscription, Window, div, px,
};
use serde_json::Value;

use crate::api::{self, download::BatchOperationRequest};
use crate::bridge::{EventHub, ToastKind, UiEvent};
use crate::components::toast;
use crate::state::{Backend, TaskCache};
use crate::util;
use crate::state::task_cache::{v_bool, v_f64, v_str, v_u64};

pub fn status_label(status: &str) -> &'static str {
    match status {
        "pending" => "等待中",
        "downloading" => "下载中",
        "uploading" => "上传中",
        "checking_rapid" => "秒传检查",
        "scanning" => "扫描中",
        "decrypting" => "解密中",
        "encrypting" => "加密中",
        "paused" => "已暂停",
        "completed" => "已完成",
        "rapid_upload_success" => "秒传成功",
        "failed" => "失败",
        "cancelled" => "已取消",
        _ => "未知",
    }
}

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

    fn rows(&self, cx: &App) -> (Vec<Value>, Vec<Value>) {
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

    fn render_row(&self, task: &Value, is_folder: bool, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let id = v_str(task, "id");
        let status = v_str(task, "status");
        let name = if is_folder { v_str(task, "name") } else { crate::api::file::basename(&v_str(task, "remote_path")) };
        let total = v_u64(task, "total_size");
        let done = v_u64(task, "downloaded_size");
        let speed = v_u64(task, "speed");
        let progress = if total > 0 { (done as f64 / total as f64 * 100.0) as f32 } else { 0.0 };
        let error = v_str(task, "error");
        let active = is_active(&status);
        let terminal = matches!(status.as_str(), "completed" | "failed" | "cancelled");
        let decrypting = status == "decrypting";
        let decrypt_progress = v_f64(task, "decrypt_progress");

        let id_pause = id.clone();
        let status_pause = status.clone();
        let id_del = id.clone();
        let id_open = id.clone();
        let local_path = v_str(task, "local_path");

        let mut row = v_flex().gap_1p5().px_3().py_2().rounded_lg().bg(theme.secondary);
        // 第一行:图标 + 名称 + 状态 + 操作
        let mut top = h_flex().gap_2().items_center();
        top = top.child(Icon::new(if is_folder { IconName::Folder } else { IconName::FileText }).size(px(18.)).text_color(theme.primary));
        top = top.child(div().flex_1().truncate().child(name));
        top = top.child(Tag::new().small().child(status_label(&status)));
        if v_bool(task, "is_encrypted") { top = top.child(Tag::new().small().child("🔒")); }
        if !terminal {
            top = top.child(Button::new(format!("dl-pr-{id}")).ghost().small()
                .icon(if active { IconName::Pause } else { IconName::Play })
                .on_click(cx.listener(move |this, _, _, cx| this.pause_resume(&id_pause, &status_pause, is_folder, cx))));
        } else if status == "failed" {
            let id_r = id.clone();
            top = top.child(Button::new(format!("dl-retry-{id}")).ghost().small().icon(IconName::RotateCw)
                .on_click(cx.listener(move |this, _, _, cx| this.pause_resume(&id_r, "paused", is_folder, cx))));
        }
        if !is_folder && !local_path.is_empty() {
            top = top.child(Button::new(format!("dl-open-{id_open}")).ghost().small().icon(IconName::FolderOpen)
                .on_click(cx.listener(move |_, _, _, _| { let _ = open::that(&local_path); })));
        }
        if is_folder {
            let id_detail = id.clone();
            top = top.child(Button::new(format!("dl-detail-{id}")).ghost().small().icon(IconName::ListTree)
                .on_click(cx.listener(move |this, _, window, cx| this.open_folder_detail(&id_detail, window, cx))));
        }
        top = top.child(Button::new(format!("dl-del-{id_del}")).ghost().small().icon(IconName::Trash)
            .on_click(cx.listener(move |this, _, window, cx| this.open_delete(&id_del, is_folder, window, cx))));
        row = row.child(top);
        // 远程路径
        row = row.child(div().text_color(theme.muted_foreground).truncate().child(v_str(task, "remote_path")));
        // 进度
        row = row.child(Progress::new(format!("dl-prog-{id}")).value(progress));
        // 统计
        let stats = if is_folder {
            format!("{}/{} 文件 · {} / {} · {}", v_u64(task, "completed_count"), v_u64(task, "total_files"), util::format_size(done), util::format_size(total), util::format_speed(speed))
        } else {
            format!("{} / {} · {}", util::format_size(done), util::format_size(total), util::format_speed(speed))
        };
        row = row.child(div().text_color(theme.muted_foreground).child(stats));
        if decrypting {
            row = row.child(Progress::new(format!("dl-dec-{id}")).value((decrypt_progress * 100.0) as f32));
        }
        if !error.is_empty() {
            row = row.child(div().text_color(theme.danger).child(error));
        }
        row
    }
}

impl Render for DownloadsView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let (folders, files) = self.rows(cx);
        let active_count = folders.iter().chain(files.iter()).filter(|t| is_active(&v_str(t, "status"))).count();

        let toolbar = h_flex().gap_2().items_center().px_4().py_1p5()
            .child(div().flex_1().text_color(theme.muted_foreground).child(format!("{active_count} 个活跃任务")))
            .child(Button::new("dl-pause-all").small().label("全部暂停").on_click(cx.listener(|this, _, _, cx| this.batch_op("pause", cx))))
            .child(Button::new("dl-resume-all").small().label("全部继续").on_click(cx.listener(|this, _, _, cx| this.batch_op("resume", cx))))
            .child(Button::new("dl-clear-done").small().label("清除已完成").on_click(cx.listener(|this, _, _, cx| this.batch_op("clear_completed", cx))))
            .child(Button::new("dl-clear-fail").small().label("清除失败").on_click(cx.listener(|this, _, _, cx| this.batch_op("clear_failed", cx))))
            .child(Button::new("dl-refresh").small().icon(IconName::RefreshCw).on_click(cx.listener(|_, _, _, cx| { cx.global::<Backend>().ws.request_snapshot(); })));

        let mut list = v_flex().gap_2().p_3().overflow_y_scrollbar();
        if folders.is_empty() && files.is_empty() {
            list = list.child(div().py_12().rounded_lg().bg(theme.secondary).text_center().text_color(theme.muted_foreground).child("暂无下载任务"));
        }
        for f in &folders {
            list = list.child(self.render_row(f, true, cx));
        }
        for f in &files {
            list = list.child(self.render_row(f, false, cx));
        }

        v_flex().flex_1().overflow_hidden()
            .child(toolbar)
            .child(div().flex_1().child(list))
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
        let theme = cx.theme().clone();
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
            list = list.child(div().py_4().text_center().text_color(theme.muted_foreground).child("暂无子任务"));
        }
        for c in &children {
            let total = v_u64(c, "total_size");
            let done = v_u64(c, "downloaded_size");
            let pct = if total > 0 { (done as f64 / total as f64 * 100.0) as f32 } else { 0.0 };
            let name = {
                let rel = v_str(c, "relative_path");
                if rel.is_empty() { crate::api::file::basename(&v_str(c, "remote_path")) } else { rel }
            };
            list = list.child(
                v_flex().gap_1().px_2().py_1p5().rounded_md().bg(theme.secondary)
                    .child(h_flex().gap_2().items_center()
                        .child(div().flex_1().truncate().child(name))
                        .child(Tag::new().small().child(status_label(&v_str(c, "status"))))
                        .child(div().text_color(theme.muted_foreground).child(util::format_size(total))))
                    .child(Progress::new(format!("fd-{}", v_str(c, "id"))).value(pct)),
            );
        }

        v_flex().gap_3()
            .child(div().text_color(theme.muted_foreground).child(stats))
            .child(list)
    }
}
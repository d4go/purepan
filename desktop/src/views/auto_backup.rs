//! 自动备份页:总览头部 + 三段式配置卡片(方向图标、状态胶囊、路径、操作),
//! 新建 / 手动备份 / 启停 / 详情(逐文件重试) / 删除。

use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::dialog::Dialog;
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::radio::RadioGroup;
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::switch::Switch;
use gpui_kit::component::tag::Tag;
use gpui_kit::component::{Icon, Sizable as _, WindowExt as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AnyElement, AnyWindowHandle, AppContext as _, Context, Entity, Hsla, InteractiveElement as _,
    IntoElement, ParentElement as _, Render, Styled as _, Subscription, Window, div, px, rgb,
};
use serde_json::json;

use crate::api::autobackup::{BackupConfig, BackupFileTask, BackupTask};
use crate::api::{self};
use crate::bridge::{EventHub, ToastKind, UiEvent};
use crate::components::task_ui::{
    circle_button, empty_state, soft_pill, status_tag, task_card,
};
use crate::components::toast;
use crate::design::{self, Palette};
use crate::state::Backend;
use crate::util;

const DIRS: [&str; 3] = ["upload", "download", "sync"];
const DIR_LABELS: [&str; 3] = ["上传备份", "下载备份", "双向同步"];

fn dir_label(d: &serde_json::Value) -> String {
    let s = d.as_str().unwrap_or("");
    match s { "upload" => "上传备份", "download" => "下载备份", "sync" => "双向同步", _ => s }.to_string()
}

fn status_str(v: &serde_json::Value) -> String {
    v.as_str().unwrap_or("").to_string()
}

/// 备份方向 → (图标, 底色, 前景色)。
fn direction_visual(dir: &str, p: &Palette) -> (IconName, Hsla, Hsla) {
    let (icon, bg, fg): (IconName, u32, u32) = match dir {
        "upload" => (IconName::CloudUpload, 0xDBEAFE, 0x2563EB),
        "download" => (IconName::CloudDownload, 0xD1FAE5, 0x059669),
        _ => (IconName::FolderSync, 0xEDE9FE, 0x7C3AED),
    };
    let fg: Hsla = rgb(fg).into();
    let bg: Hsla = if p.is_dark { fg.alpha(0.16) } else { rgb(bg).into() };
    (icon, bg, fg)
}

pub struct AutoBackupView {
    configs: Vec<BackupConfig>,
    name_input: Entity<InputState>,
    local_input: Entity<InputState>,
    remote_input: Entity<InputState>,
    direction: usize,
    encrypt: bool,
    window_handle: AnyWindowHandle,
    _subscriptions: Vec<Subscription>,
}

impl AutoBackupView {
    pub fn new(hub: Entity<EventHub>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let name_input = cx.new(|cx| InputState::new(window, cx).placeholder("配置名称"));
        let local_input = cx.new(|cx| InputState::new(window, cx).placeholder("本地目录绝对路径"));
        let remote_input = cx.new(|cx| {
            let mut s = InputState::new(window, cx).placeholder("网盘目录");
            s.set_value("/".to_string(), window, cx);
            s
        });
        let sub = cx.subscribe(&hub, |this, _, ev, cx| {
            if matches!(ev, UiEvent::WsMessage(_) | UiEvent::AccountChanged { .. }) { this.refresh(cx); }
        });
        let mut v = Self { configs: Vec::new(), name_input, local_input, remote_input, direction: 0, encrypt: false, window_handle: window.window_handle(), _subscriptions: vec![sub] };
        v.refresh(cx);
        v
    }

    fn toast(&self, cx: &mut Context<Self>, kind: ToastKind, msg: impl Into<String>) {
        let msg = msg.into();
        let wh = self.window_handle;
        let _ = wh.update(cx, |_, w, cx| toast::show(w, cx, kind, msg.clone()));
    }

    fn refresh(&mut self, cx: &mut Context<Self>) {
        let b = cx.global::<Backend>();
        let (rt, api) = (b.rt.clone(), b.api.clone());
        cx.spawn(async move |this, cx| {
            let a = api.clone();
            let r = api::call_api(&rt, move || async move { a.list_backup_configs().await }).await;
            let _ = this.update(cx, |this, cx| {
                if let Ok(c) = r { this.configs = c; }
                cx.notify();
            });
        }).detach();
    }

    fn open_create(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let view = cx.entity().clone();
        let name = self.name_input.clone();
        let local = self.local_input.clone();
        let remote = self.remote_input.clone();
        let dir = self.direction;
        let enc = self.encrypt;
        let v2 = view.clone();
        let v3 = view.clone();
        let v4 = view.clone();
        window.open_dialog(cx, move |d: Dialog, _, _| {
            let v5 = v2.clone();
            let v6 = v3.clone();
            let v7 = v4.clone();
            d.title("新建备份配置").width(px(520.))
                .child(v_flex().gap_3()
                    .child(h_flex().gap_2().items_center().child("名称").child(Input::new(&name).large()))
                    .child(h_flex().gap_2().items_center().child("本地目录").child(Input::new(&local).large()))
                    .child(h_flex().gap_2().items_center().child("网盘目录").child(Input::new(&remote).large()))
                    .child(h_flex().gap_2().items_center().child("方向").child(
                        RadioGroup::new("ab-dir").children(DIR_LABELS).selected_index(Some(dir))
                            .on_change(move |i, _, cx| { v5.update(cx, |this, cx| { this.direction = *i; cx.notify(); }); })))
                    .child(Switch::new("ab-enc").label("启用加密").checked(enc)
                        .on_change(move |c, _, cx| { v6.update(cx, |this, cx| { this.encrypt = *c; cx.notify(); }); })))
                .footer(h_flex().gap_2().justify_end()
                    .child(Button::new("ab-cancel").label("取消").on_click(|_, w, cx| w.close_dialog(cx)))
                    .child(Button::new("ab-create").primary().label("创建").on_click(move |_, window, cx| { v7.update(cx, |this, cx| this.create(window, cx)); })))
        });
    }

    fn create(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let name = self.name_input.read(cx).value().trim().to_string();
        let local = self.local_input.read(cx).value().trim().to_string();
        let remote = self.remote_input.read(cx).value().trim().to_string();
        if name.is_empty() || local.is_empty() { toast::show(window, cx, ToastKind::Warning, "请填写名称和本地目录"); return; }
        let body = json!({
            "name": name, "local_path": local, "remote_path": remote,
            "direction": DIRS[self.direction], "encrypt_enabled": self.encrypt,
        });
        let b = cx.global::<Backend>();
        let (rt, api) = (b.rt.clone(), b.api.clone());
        let wh = self.window_handle;
        cx.spawn(async move |this, cx| {
            let (a, body2) = (api.clone(), body.clone());
            let r = api::call_api(&rt, move || async move { a.create_backup_config(body2).await }).await;
            let _ = this.update(cx, |this, cx| {
                match r {
                    Ok(_) => { let _ = wh.update(cx, |_, w, cx| w.close_all_dialogs(cx)); this.toast(cx, ToastKind::Success, "配置已创建"); this.refresh(cx); }
                    Err(e) => this.toast(cx, ToastKind::Error, e.message()),
                }
            });
        }).detach();
    }

    fn trigger(&mut self, id: &str, cx: &mut Context<Self>) {
        let b = cx.global::<Backend>();
        let (rt, api) = (b.rt.clone(), b.api.clone());
        let id = id.to_string();
        cx.spawn(async move |this, cx| {
            let (a, i) = (api.clone(), id.clone());
            let r = api::call_api(&rt, move || async move { a.trigger_backup(&i).await }).await;
            let _ = this.update(cx, |this, cx| {
                match r { Ok(_) => this.toast(cx, ToastKind::Success, "已触发备份"), Err(e) => this.toast(cx, ToastKind::Error, e.message()) }
            });
        }).detach();
    }

    fn toggle_enabled(&mut self, id: &str, enabled: bool, cx: &mut Context<Self>) {
        let b = cx.global::<Backend>();
        let (rt, api) = (b.rt.clone(), b.api.clone());
        let id = id.to_string();
        cx.spawn(async move |this, cx| {
            let (a, i) = (api.clone(), id.clone());
            let r = if enabled {
                api::call_api(&rt, move || async move { a.disable_backup_config(&i).await }).await
            } else {
                api::call_api(&rt, move || async move { a.enable_backup_config(&i).await }).await
            };
            let _ = this.update(cx, |this, cx| { if let Err(e) = r { this.toast(cx, ToastKind::Error, e.message()); } else { this.refresh(cx); } });
        }).detach();
    }

    fn delete(&mut self, id: &str, cx: &mut Context<Self>) {
        let b = cx.global::<Backend>();
        let (rt, api) = (b.rt.clone(), b.api.clone());
        let id = id.to_string();
        cx.spawn(async move |this, cx| {
            let (a, i) = (api.clone(), id.clone());
            let r = api::call_api(&rt, move || async move { a.delete_backup_config(&i).await }).await;
            let _ = this.update(cx, |this, cx| { if let Err(e) = r { this.toast(cx, ToastKind::Error, e.message()); } else { this.refresh(cx); } });
        }).detach();
    }

    fn open_detail(&mut self, config_id: &str, window: &mut Window, cx: &mut Context<Self>) {
        let config_id = config_id.to_string();
        let detail = cx.new(|cx| BackupDetail::new(config_id.clone(), cx));
        window.open_dialog(cx, move |d: Dialog, _, _| d.title("备份任务详情").width(px(680.)).child(detail.clone()));
    }

    fn render_row(&self, c: &BackupConfig, p: &Palette, cx: &mut Context<Self>) -> AnyElement {
        let id = c.id.clone();
        let dir = dir_label(&serde_json::to_value(&c.direction).unwrap_or_default());
        let dir_key = serde_json::to_value(&c.direction).ok().and_then(|v| v.as_str().map(String::from)).unwrap_or_default();
        let enabled = c.enabled;
        let encrypt = c.encrypt_enabled;

        let (icon, bg, fg) = direction_visual(&dir_key, p);
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

        let mut title = h_flex().gap(px(8.)).items_center().min_w_0().overflow_hidden();
        title = title.child(
            div()
                .flex_1()
                .min_w_0()
                .truncate()
                .text_size(px(14.))
                .font_weight(gpui_kit::FontWeight::MEDIUM)
                .child(c.name.clone()),
        );
        title = title.child(soft_pill(dir, p.selected_bg, p.muted));
        title = title.child(if enabled {
            soft_pill(
                "已启用",
                if p.is_dark { p.primary.alpha(0.16) } else { rgb(0xD1FAE5).into() },
                if p.is_dark { rgb(0x34D399).into() } else { rgb(0x059669).into() },
            )
        } else {
            soft_pill("已停用", p.selected_bg, p.muted)
        });
        if encrypt {
            title = title.child(div().text_size(px(11.)).child("🔒"));
        }

        let mut mid = v_flex().flex_1().min_w_0().gap(px(6.)).overflow_hidden().child(title);

        let mut meta = h_flex()
            .gap(px(6.))
            .items_center()
            .min_w_0()
            .overflow_hidden()
            .text_size(px(12.))
            .text_color(p.faint);
        meta = meta.child(div().truncate().child(c.local_path.display().to_string()));
        meta = meta.child(div().text_color(p.faint.alpha(0.8)).child("→"));
        meta = meta.child(div().truncate().child(c.remote_path.clone()));
        mid = mid.child(meta);

        let mut actions = h_flex().gap(px(2.)).items_center().flex_shrink_0();
        let call = |key: &str| gpui_kit::SharedString::from(format!("{key}:{id}"));
        let id_trigger = id.clone();
        actions = actions.child(circle_button(p, call("ab-trigger"), IconName::Play, move |this, _, cx| {
            this.trigger(&id_trigger, cx)
        }, cx));
        let id_toggle = id.clone();
        actions = actions.child(circle_button(p, call("ab-toggle"), if enabled { IconName::CirclePause } else { IconName::CirclePlay }, move |this, _, cx| {
            this.toggle_enabled(&id_toggle, enabled, cx)
        }, cx));
        let id_detail = id.clone();
        actions = actions.child(circle_button(p, call("ab-detail"), IconName::ListTree, move |this, window, cx| {
            this.open_detail(&id_detail, window, cx)
        }, cx));
        let id_del = id.clone();
        actions = actions.child(circle_button(p, call("ab-del"), IconName::Trash, move |this, _, cx| {
            this.delete(&id_del, cx)
        }, cx));

        task_card(p, tile, mid.into_any_element(), actions.into_any_element())
    }
}

impl Render for AutoBackupView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = design::palette(cx);
        let enabled_count = self.configs.iter().filter(|c| c.enabled).count();

        let status_pill = if enabled_count > 0 {
            soft_pill(format!("{enabled_count} 个已启用"), p.primary_soft, p.primary)
        } else if !self.configs.is_empty() {
            soft_pill(format!("{} 个配置", self.configs.len()), p.primary_soft, p.primary)
        } else {
            Tag::new().small().rounded_full().child("暂无配置")
        };

        let header = h_flex()
            .h(px(68.))
            .px(px(32.))
            .gap(px(12.))
            .items_center()
            .flex_shrink_0()
            .child(div().text_size(px(18.)).font_weight(gpui_kit::FontWeight::SEMIBOLD).child("自动备份"))
            .child(status_pill)
            .child(div().flex_1())
            .child(
                h_flex()
                    .gap(px(6.))
                    .items_center()
                    .child(
                        Button::new("ab-new")
                            .primary()
                            .small()
                            .icon(IconName::Plus)
                            .label("新建")
                            .on_click(cx.listener(|this, _, window, cx| this.open_create(window, cx))),
                    )
                    .child(
                        Button::new("ab-refresh")
                            .ghost()
                            .small()
                            .icon(IconName::RefreshCw)
                            .on_click(cx.listener(|this, _, _, cx| this.refresh(cx))),
                    ),
            )
            .into_any_element();

        let body: AnyElement = if self.configs.is_empty() {
            empty_state(
                &p,
                IconName::RefreshCw,
                "暂无备份配置",
                "为本地目录与网盘目录建立自动备份关系",
                Some(
                    Button::new("ab-empty-new")
                        .small()
                        .icon(IconName::Plus)
                        .label("新建配置")
                        .on_click(cx.listener(|this, _, window, cx| this.open_create(window, cx)))
                        .into_any_element(),
                ),
            )
        } else {
            let mut list = v_flex()
                .id("ab-scroll")
                .flex_1()
                .min_h_0()
                .gap(px(10.))
                .px(px(32.))
                .py(px(16.))
                .overflow_y_scrollbar();
            for c in &self.configs {
                list = list.child(self.render_row(c, &p, cx));
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

// ───────────────────────── 任务详情 ─────────────────────────

struct BackupDetail {
    config_id: String,
    tasks: Vec<BackupTask>,
    file_tasks: Vec<BackupFileTask>,
    selected_task: Option<String>,
}

impl BackupDetail {
    fn new(config_id: String, cx: &mut Context<Self>) -> Self {
        let mut d = Self { config_id, tasks: Vec::new(), file_tasks: Vec::new(), selected_task: None };
        d.load_tasks(cx);
        d
    }

    fn load_tasks(&mut self, cx: &mut Context<Self>) {
        let b = cx.global::<Backend>();
        let (rt, api) = (b.rt.clone(), b.api.clone());
        let cid = self.config_id.clone();
        cx.spawn(async move |this, cx| {
            let (a, c) = (api.clone(), cid.clone());
            let r = api::call_api(&rt, move || async move { a.list_backup_tasks(&c, 1).await }).await;
            let _ = this.update(cx, |this, cx| {
                if let Ok(resp) = r {
                    this.tasks = resp.tasks;
                    if let Some(first) = this.tasks.first() {
                        let tid = first.id.clone();
                        this.selected_task = Some(tid.clone());
                        this.load_files(tid, cx);
                    }
                }
                cx.notify();
            });
        }).detach();
    }

    fn load_files(&mut self, task_id: String, cx: &mut Context<Self>) {
        let b = cx.global::<Backend>();
        let (rt, api) = (b.rt.clone(), b.api.clone());
        cx.spawn(async move |this, cx| {
            let (a, t) = (api.clone(), task_id.clone());
            let r = api::call_api(&rt, move || async move { a.list_file_tasks(&t, 1).await }).await;
            let _ = this.update(cx, |this, cx| {
                if let Ok(resp) = r { this.file_tasks = resp.file_tasks; }
                cx.notify();
            });
        }).detach();
    }

    fn retry(&mut self, file_id: &str, cx: &mut Context<Self>) {
        let Some(tid) = self.selected_task.clone() else { return };
        let b = cx.global::<Backend>();
        let (rt, api) = (b.rt.clone(), b.api.clone());
        let fid = file_id.to_string();
        cx.spawn(async move |this, cx| {
            let (a, t, f) = (api.clone(), tid.clone(), fid.clone());
            let _ = api::call_api(&rt, move || async move { a.retry_file_task(&t, &f).await }).await;
            let _ = this.update(cx, |this, cx| { this.load_files(tid.clone(), cx); });
        }).detach();
    }
}

impl Render for BackupDetail {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = design::palette(cx);
        let mut tasks_sel = h_flex().gap(px(6.)).overflow_x_scrollbar();
        for t in &self.tasks {
            let tid = t.id.clone();
            let selected = self.selected_task.as_deref() == Some(t.id.as_str());
            let status = status_str(&serde_json::to_value(&t.status).unwrap_or_default());
            tasks_sel = tasks_sel.child(
                Button::new(format!("bd-task-{}", t.id)).small()
                    .when(selected, |b| b.primary())
                    .label(format!("{} ({})", util::format_timestamp(t.created_at.timestamp()), status))
                    .on_click(cx.listener(move |this, _, _, cx| { this.selected_task = Some(tid.clone()); this.load_files(tid.clone(), cx); })),
            );
        }
        let mut files = v_flex().max_h(px(360.)).overflow_y_scrollbar();
        if self.file_tasks.is_empty() {
            files = files.child(div().py_4().text_center().text_color(p.muted).child("暂无文件任务"));
        }
        for f in &self.file_tasks {
            let fid = f.id.clone();
            let status = status_str(&serde_json::to_value(&f.status).unwrap_or_default());
            let name = crate::api::file::basename(&f.remote_path);
            let failed = status == "failed";
            let failed_retry = failed;
            files = files.child(
                h_flex().gap(px(8.)).items_center().px(px(10.)).py(px(8.))
                    .border_b_1().border_color(p.border)
                    .child(div().flex_1().truncate().child(name))
                    .child(status_tag(&status, &p))
                    .child(div().text_size(px(12.)).text_color(p.faint).child(util::format_size(f.file_size)))
                    .when(failed_retry, |this| this.child(circle_button(
                        &p,
                        gpui_kit::SharedString::from(format!("bd-retry-{}", f.id)),
                        IconName::RotateCw,
                        move |this, _, cx| this.retry(&fid, cx),
                        cx,
                    ))),
            );
        }
        v_flex().gap(px(12.)).child(tasks_sel).child(files)
    }
}
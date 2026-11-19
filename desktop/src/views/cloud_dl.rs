//! 离线下载页:总览头部 + 三段式任务卡片(类型图标、进度、元信息、操作),
//! 新建(磁力/HTTP)/取消/删除/清空。

use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::dialog::Dialog;
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::switch::Switch;
use gpui_kit::component::tag::Tag;
use gpui_kit::component::{Disableable as _, Icon, Sizable as _, WindowExt as _, h_flex, v_flex};
use gpui_kit::{
    AnyElement, AnyWindowHandle, AppContext as _, Context, Entity, InteractiveElement as _,
    IntoElement, ParentElement as _, Render, Styled as _, Subscription, Window, div, px,
};

use crate::api::cloud_dl::{AddTaskRequest, CloudDlTaskInfo};
use crate::api::{self};
use crate::bridge::{EventHub, ToastKind, UiEvent};
use crate::components::task_ui::{circle_button, empty_state, progress_bar, sep_dot, soft_pill, status_tag, task_card};
use crate::components::toast;
use crate::design::{self, Palette};
use crate::state::Backend;
use crate::util;

/// 百度离线任务状态码 → 通用状态键(用于胶囊配色/文案)。
fn status_key(code: i32) -> &'static str {
    match code {
        0 => "downloading",
        1 => "pending",
        2 => "paused",
        3 => "completed",
        4 => "failed",
        5 => "cancelled",
        _ => "unknown",
    }
}

fn is_cancellable(code: i32) -> bool {
    matches!(code, 0 | 1)
}

pub struct CloudDlView {
    tasks: Vec<CloudDlTaskInfo>,
    url_input: Entity<InputState>,
    save_input: Entity<InputState>,
    auto_download: bool,
    window_handle: AnyWindowHandle,
    _subscriptions: Vec<Subscription>,
}

impl CloudDlView {
    pub fn new(hub: Entity<EventHub>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let url_input = cx.new(|cx| InputState::new(window, cx).placeholder("HTTP/HTTPS/磁力链接/ed2k"));
        let save_input = cx.new(|cx| {
            let mut s = InputState::new(window, cx).placeholder("网盘保存路径");
            s.set_value("/".to_string(), window, cx);
            s
        });
        let sub = cx.subscribe(&hub, |this, _, ev, cx| {
            if matches!(ev, UiEvent::WsMessage(_) | UiEvent::AccountChanged { .. }) { this.refresh(cx); }
        });
        let mut v = Self { tasks: Vec::new(), url_input, save_input, auto_download: false, window_handle: window.window_handle(), _subscriptions: vec![sub] };
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
            let r = api::call_api(&rt, move || async move { a.cloud_dl_list().await }).await;
            let _ = this.update(cx, |this, cx| {
                if let Ok(data) = r { this.tasks = data.tasks; }
                cx.notify();
            });
        }).detach();
    }

    fn open_add(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let view = cx.entity().clone();
        let url = self.url_input.clone();
        let save = self.save_input.clone();
        let auto_dl = self.auto_download;
        let v2 = view.clone();
        window.open_dialog(cx, move |d: Dialog, _, _| {
            let v4 = v2.clone();
            d.title("新建离线下载").width(px(500.))
                .child(v_flex().gap_3()
                    .child(h_flex().gap_2().items_center().child("链接").child(Input::new(&url).large()))
                    .child(h_flex().gap_2().items_center().child("保存路径").child(Input::new(&save).large()))
                    .child(Switch::new("cd-auto").label("完成后自动下载到本地").checked(auto_dl)
                        .on_change(move |c, _, cx| { v4.update(cx, |this, cx| { this.auto_download = *c; cx.notify(); }); })))
                .footer(h_flex().gap_2().justify_end()
                    .child(Button::new("cd-cancel").label("取消").on_click(|_, w, cx| w.close_dialog(cx)))
                    .child(Button::new("cd-add").primary().label("添加").on_click({ let v = v2.clone(); move |_, window, cx| { v.update(cx, |this, cx| this.add_task(window, cx)); } })))
        });
    }

    fn add_task(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let url = self.url_input.read(cx).value().trim().to_string();
        if url.is_empty() { toast::show(window, cx, ToastKind::Warning, "请输入链接"); return; }
        let save = self.save_input.read(cx).value().trim().to_string();
        let req = AddTaskRequest {
            source_url: url,
            save_path: Some(if save.is_empty() { "/".into() } else { save }),
            auto_download: Some(self.auto_download),
            local_download_path: None,
            ask_download_path: None,
        };
        let b = cx.global::<Backend>();
        let (rt, api) = (b.rt.clone(), b.api.clone());
        let wh = self.window_handle;
        cx.spawn(async move |this, cx| {
            let (a, r) = (api.clone(), req.clone());
            let res = api::call_api(&rt, move || async move { a.cloud_dl_add(&r).await }).await;
            let _ = this.update(cx, |this, cx| {
                match res {
                    Ok(_) => {
                        let _ = wh.update(cx, |_, w, cx| w.close_all_dialogs(cx));
                        this.toast(cx, ToastKind::Success, "离线任务已添加");
                        this.refresh(cx);
                    }
                    Err(e) => this.toast(cx, ToastKind::Error, e.message()),
                }
            });
        }).detach();
    }

    fn cancel(&mut self, id: u64, cx: &mut Context<Self>) {
        let b = cx.global::<Backend>();
        let (rt, api) = (b.rt.clone(), b.api.clone());
        cx.spawn(async move |this, cx| {
            let a = api.clone();
            let r = api::call_api(&rt, move || async move { a.cloud_dl_cancel(id).await }).await;
            let _ = this.update(cx, |this, cx| { if let Err(e) = r { this.toast(cx, ToastKind::Error, e.message()); } else { this.refresh(cx); } });
        }).detach();
    }

    fn delete(&mut self, id: u64, cx: &mut Context<Self>) {
        let b = cx.global::<Backend>();
        let (rt, api) = (b.rt.clone(), b.api.clone());
        cx.spawn(async move |this, cx| {
            let a = api.clone();
            let r = api::call_api(&rt, move || async move { a.cloud_dl_delete(id).await }).await;
            let _ = this.update(cx, |this, cx| { if let Err(e) = r { this.toast(cx, ToastKind::Error, e.message()); } else { this.refresh(cx); } });
        }).detach();
    }

    fn clear(&mut self, cx: &mut Context<Self>) {
        let b = cx.global::<Backend>();
        let (rt, api) = (b.rt.clone(), b.api.clone());
        cx.spawn(async move |this, cx| {
            let a = api.clone();
            let r = api::call_api(&rt, move || async move { a.cloud_dl_clear().await }).await;
            let _ = this.update(cx, |this, cx| { if let Err(e) = r { this.toast(cx, ToastKind::Error, e.message()); } else { this.refresh(cx); } });
        }).detach();
    }

    fn render_row(&self, t: &CloudDlTaskInfo, p: &Palette, cx: &mut Context<Self>) -> AnyElement {
        let id = t.task_id as u64;
        let key = status_key(t.status);
        let total = t.file_size.max(0) as u64;
        let done = t.finished_size.max(0) as u64;
        let pct = if total > 0 { (done as f64 / total as f64) as f32 } else { 0.0 };
        let completed = t.status == 3;

        let (icon, bg, fg) = design::file_visual(&t.task_name, p.is_dark);
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
                .child(t.task_name.clone()),
        );
        title = title.child(status_tag(key, p));
        if t.file_list.len() > 1 {
            title = title.child(soft_pill(format!("{} 个文件", t.file_list.len()), p.selected_bg, p.muted));
        }

        let mut mid = v_flex().flex_1().min_w_0().gap(px(6.)).overflow_hidden().child(title);

        if !completed {
            let fill = if is_cancellable(t.status) { p.primary } else { p.faint.alpha(0.5) };
            mid = mid.child(progress_bar(pct, fill, p.faint.alpha(0.18)));
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
        }
        if !t.save_path.is_empty() {
            meta = meta.child(sep_dot(p));
            meta = meta.child(div().truncate().child(format!("保存至 {}", t.save_path)));
        }
        mid = mid.child(meta);

        let mut actions = h_flex().gap(px(2.)).items_center().flex_shrink_0();
        if is_cancellable(t.status) {
            actions = actions.child(circle_button(
                p,
                gpui_kit::SharedString::from(format!("cd-cancel:{id}")),
                IconName::CircleStop,
                move |this, _, cx| this.cancel(id, cx),
                cx,
            ));
        }
        actions = actions.child(circle_button(
            p,
            gpui_kit::SharedString::from(format!("cd-del:{id}")),
            IconName::Trash,
            move |this, _, cx| this.delete(id, cx),
            cx,
        ));

        task_card(p, tile, mid.into_any_element(), actions.into_any_element())
    }
}

impl Render for CloudDlView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = design::palette(cx);
        let count = self.tasks.len();
        let active_count = self.tasks.iter().filter(|t| is_cancellable(t.status)).count();

        let status_pill = if active_count > 0 {
            soft_pill(format!("{active_count} 个进行中"), p.primary_soft, p.primary)
        } else if count > 0 {
            soft_pill(format!("{count} 个任务"), p.primary_soft, p.primary)
        } else {
            Tag::new().small().rounded_full().child("暂无任务")
        };

        let header = h_flex()
            .h(px(68.))
            .px(px(32.))
            .gap(px(12.))
            .items_center()
            .flex_shrink_0()
            .child(div().text_size(px(18.)).font_weight(gpui_kit::FontWeight::SEMIBOLD).child("离线下载"))
            .child(status_pill)
            .child(div().flex_1())
            .child(
                h_flex()
                    .gap(px(6.))
                    .items_center()
                    .child(
                        Button::new("cd-new")
                            .primary()
                            .small()
                            .icon(IconName::Plus)
                            .label("新建")
                            .on_click(cx.listener(|this, _, window, cx| this.open_add(window, cx))),
                    )
                    .child(
                        Button::new("cd-clear")
                            .ghost()
                            .small()
                            .icon(IconName::Eraser)
                            .label("清空")
                            .disabled(count == 0)
                            .on_click(cx.listener(|this, _, _, cx| this.clear(cx))),
                    )
                    .child(
                        Button::new("cd-refresh")
                            .ghost()
                            .small()
                            .icon(IconName::RefreshCw)
                            .on_click(cx.listener(|this, _, _, cx| this.refresh(cx))),
                    ),
            )
            .into_any_element();

        let body: AnyElement = if self.tasks.is_empty() {
            empty_state(
                &p,
                IconName::CloudDownload,
                "暂无离线任务",
                "粘贴磁力或 HTTP 链接,让网盘替你下载",
                Some(
                    Button::new("cd-empty-new")
                        .small()
                        .icon(IconName::Plus)
                        .label("新建任务")
                        .on_click(cx.listener(|this, _, window, cx| this.open_add(window, cx)))
                        .into_any_element(),
                ),
            )
        } else {
            let mut list = v_flex()
                .id("cd-scroll")
                .flex_1()
                .min_h_0()
                .gap(px(10.))
                .px(px(32.))
                .py(px(16.))
                .overflow_y_scrollbar();
            for t in &self.tasks {
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
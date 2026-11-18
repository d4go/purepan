//! 离线下载页(移植 CloudDlView.vue):任务列表,新建(磁力/HTTP),取消/删除/详情/清空。

use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::dialog::Dialog;
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::progress::Progress;
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::switch::Switch;
use gpui_kit::component::tag::Tag;
use gpui_kit::component::{ActiveTheme as _, Icon, Sizable as _, WindowExt as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AnyWindowHandle, App, AppContext as _, Context, Entity, IntoElement, ParentElement as _,
    Render, Styled as _, Subscription, Window, div, px,
};

use crate::api::cloud_dl::{AddTaskRequest, CloudDlTaskInfo};
use crate::api::{self};
use crate::bridge::{EventHub, ToastKind, UiEvent};
use crate::components::toast;
use crate::state::Backend;
use crate::util;

pub struct CloudDlView {
    tasks: Vec<CloudDlTaskInfo>,
    loading: bool,
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
        let mut v = Self { tasks: Vec::new(), loading: false, url_input, save_input, auto_download: false, window_handle: window.window_handle(), _subscriptions: vec![sub] };
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

    fn open_add(&mut self, cx: &mut Context<Self>) {
        let view = cx.entity().clone();
        let url = self.url_input.clone();
        let save = self.save_input.clone();
        let wh = self.window_handle;
        let auto_dl = self.auto_download;
        let _ = wh.update(cx, move |_, window, cx| {
            let v2 = view.clone();
            window.open_dialog(cx, move |d: Dialog, _, _| {
                let v4 = v2.clone();
                d.title("新建离线下载").width(px(500.))
                    .child(v_flex().gap_3()
                        .child(h_flex().gap_2().items_center().child("链接").child(Input::new(&url)))
                        .child(h_flex().gap_2().items_center().child("保存路径").child(Input::new(&save)))
                        .child(Switch::new("cd-auto").label("完成后自动下载到本地").checked(auto_dl)
                            .on_change(move |c, _, cx| { v4.update(cx, |this, cx| { this.auto_download = *c; cx.notify(); }); })))
                    .footer(h_flex().gap_2().justify_end()
                        .child(Button::new("cd-cancel").label("取消").on_click(|_, w, cx| w.close_dialog(cx)))
                        .child(Button::new("cd-add").primary().label("添加").on_click({ let v = v2.clone(); move |_, _, cx| { v.update(cx, |this, cx| this.add_task(cx)); } })))
            });
        });
    }

    fn add_task(&mut self, cx: &mut Context<Self>) {
        let url = self.url_input.read(cx).value().trim().to_string();
        if url.is_empty() { self.toast(cx, ToastKind::Warning, "请输入链接"); return; }
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
}

impl Render for CloudDlView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let toolbar = h_flex().gap_2().items_center().px_4().py_2().border_b_1().border_color(theme.border)
            .child(div().flex_1().child("离线下载"))
            .child(Button::new("cd-new").primary().small().icon(IconName::Plus).label("新建").on_click(cx.listener(|this, _, _, cx| this.open_add(cx))))
            .child(Button::new("cd-clear").small().label("清空").on_click(cx.listener(|this, _, _, cx| this.clear(cx))))
            .child(Button::new("cd-refresh").small().icon(IconName::RefreshCw).on_click(cx.listener(|this, _, _, cx| this.refresh(cx))));

        let mut list = v_flex().overflow_y_scrollbar();
        if self.tasks.is_empty() {
            list = list.child(div().py_8().text_center().text_color(theme.muted_foreground).child("暂无离线任务"));
        }
        for t in &self.tasks {
            let id = t.task_id as u64;
            let pct = if t.file_size > 0 { (t.finished_size as f64 / t.file_size as f64 * 100.0) as f32 } else { 0.0 };
            let status_text = t.status_text.clone();
            let name = t.task_name.clone();
            let save_path = t.save_path.clone();
            let size = t.file_size as u64;
            let finished = t.finished_size as u64;
            // status 0/1 进行中可取消;其余可删除
            let cancellable = t.status == 0 || t.status == 1;
            let id_cancel = id;
            let id_del = id;
            let mut row = v_flex().gap_1().px_4().py_2().border_b_1().border_color(theme.border);
            let mut top = h_flex().gap_2().items_center();
            top = top.child(Icon::new(IconName::CloudDownload).size(px(18.)).text_color(theme.primary));
            top = top.child(div().flex_1().truncate().child(name));
            top = top.child(Tag::new().small().child(status_text));
            if cancellable {
                top = top.child(Button::new(format!("cd-cancel-{id}")).ghost().small().icon(IconName::CircleStop)
                    .on_click(cx.listener(move |this, _, _, cx| this.cancel(id_cancel, cx))));
            }
            top = top.child(Button::new(format!("cd-del-{id}")).ghost().small().icon(IconName::Trash)
                .on_click(cx.listener(move |this, _, _, cx| this.delete(id_del, cx))));
            row = row.child(top);
            row = row.child(div().text_color(theme.muted_foreground).truncate().child(format!("保存至: {save_path}")));
            row = row.child(Progress::new(format!("cd-prog-{id}")).value(pct));
            row = row.child(div().text_color(theme.muted_foreground).child(format!("{} / {}", util::format_size(finished), util::format_size(size))));
            list = list.child(row);
        }
        v_flex().flex_1().overflow_hidden().child(toolbar).child(div().flex_1().child(list))
    }
}
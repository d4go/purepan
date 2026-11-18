//! 分享同步页(移植 ShareSyncView.vue):订阅卡 CRUD、立即同步/恢复/启停、运行历史。

use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::dialog::Dialog;
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::tag::Tag;
use gpui_kit::component::{ActiveTheme as _, Icon, Sizable as _, WindowExt as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AnyWindowHandle, App, AppContext as _, Context, Entity, IntoElement, ParentElement as _,
    Render, Styled as _, Subscription, Window, div, px,
};
use serde_json::json;

use crate::api::share_sync::{RunRecord, ShareSubscription};
use crate::api::{self};
use crate::bridge::{EventHub, ToastKind, UiEvent};
use crate::components::toast;
use crate::state::Backend;
use crate::util;

pub struct ShareSyncView {
    subs: Vec<ShareSubscription>,
    name_input: Entity<InputState>,
    url_input: Entity<InputState>,
    pwd_input: Entity<InputState>,
    path_input: Entity<InputState>,
    window_handle: AnyWindowHandle,
    _subscriptions: Vec<Subscription>,
}

impl ShareSyncView {
    pub fn new(hub: Entity<EventHub>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let name_input = cx.new(|cx| InputState::new(window, cx).placeholder("订阅名称"));
        let url_input = cx.new(|cx| InputState::new(window, cx).placeholder("分享链接"));
        let pwd_input = cx.new(|cx| InputState::new(window, cx).placeholder("提取码(可选)"));
        let path_input = cx.new(|cx| {
            let mut s = InputState::new(window, cx).placeholder("网盘同步目录");
            s.set_value("/".to_string(), window, cx);
            s
        });
        let sub = cx.subscribe(&hub, |this, _, ev, cx| {
            if matches!(ev, UiEvent::WsMessage(_) | UiEvent::AccountChanged { .. }) { this.refresh(cx); }
        });
        let mut v = Self { subs: Vec::new(), name_input, url_input, pwd_input, path_input, window_handle: window.window_handle(), _subscriptions: vec![sub] };
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
            let r = api::call_api(&rt, move || async move { a.list_subscriptions().await }).await;
            let _ = this.update(cx, |this, cx| { if let Ok(s) = r { this.subs = s; } cx.notify(); });
        }).detach();
    }

    fn open_create(&mut self, cx: &mut Context<Self>) {
        let view = cx.entity().clone();
        let name = self.name_input.clone();
        let url = self.url_input.clone();
        let pwd = self.pwd_input.clone();
        let path = self.path_input.clone();
        let wh = self.window_handle;
        let _ = wh.update(cx, move |_, window, cx| {
            let v2 = view.clone();
            window.open_dialog(cx, move |d: Dialog, _, _| {
                let v3 = v2.clone();
                d.title("新建分享订阅").width(px(520.))
                    .child(v_flex().gap_3()
                        .child(h_flex().gap_2().items_center().child("名称").child(Input::new(&name)))
                        .child(h_flex().gap_2().items_center().child("分享链接").child(Input::new(&url)))
                        .child(h_flex().gap_2().items_center().child("提取码").child(Input::new(&pwd)))
                        .child(h_flex().gap_2().items_center().child("网盘目录").child(Input::new(&path))))
                    .footer(h_flex().gap_2().justify_end()
                        .child(Button::new("ss-cancel").label("取消").on_click(|_, w, cx| w.close_dialog(cx)))
                        .child(Button::new("ss-create").primary().label("创建").on_click(move |_, _, cx| { v3.update(cx, |this, cx| this.create(cx)); })))
            });
        });
    }

    fn create(&mut self, cx: &mut Context<Self>) {
        let name = self.name_input.read(cx).value().trim().to_string();
        let url = self.url_input.read(cx).value().trim().to_string();
        let pwd = self.pwd_input.read(cx).value().trim().to_string();
        let path = self.path_input.read(cx).value().trim().to_string();
        if name.is_empty() || url.is_empty() { self.toast(cx, ToastKind::Warning, "请填写名称和分享链接"); return; }
        let body = json!({
            "name": name, "share_url": url,
            "password": if pwd.is_empty() { None } else { Some(pwd) },
            "targets": [{ "kind": "netdisk", "remote_path": if path.is_empty() { "/".to_string() } else { path } }],
        });
        let b = cx.global::<Backend>();
        let (rt, api) = (b.rt.clone(), b.api.clone());
        let wh = self.window_handle;
        cx.spawn(async move |this, cx| {
            let (a, body2) = (api.clone(), body.clone());
            let r = api::call_api(&rt, move || async move { a.create_subscription(body2).await }).await;
            let _ = this.update(cx, |this, cx| {
                match r {
                    Ok(_) => { let _ = wh.update(cx, |_, w, cx| w.close_all_dialogs(cx)); this.toast(cx, ToastKind::Success, "订阅已创建"); this.refresh(cx); }
                    Err(e) => this.toast(cx, ToastKind::Error, e.message()),
                }
            });
        }).detach();
    }

    fn act(&mut self, id: &str, op: &'static str, cx: &mut Context<Self>) {
        let b = cx.global::<Backend>();
        let (rt, api) = (b.rt.clone(), b.api.clone());
        let id = id.to_string();
        cx.spawn(async move |this, cx| {
            let (a, i) = (api.clone(), id.clone());
            let r: Result<(), api::ApiError> = match op {
                "trigger" => api::call_api(&rt, move || async move { a.trigger_subscription(&i).await.map(|_| ()) }).await,
                "resume" => api::call_api(&rt, move || async move { a.resume_subscription(&i).await.map(|_| ()) }).await,
                "enable" => api::call_api(&rt, move || async move { a.set_subscription_enabled(&i, true).await.map(|_| ()) }).await,
                "disable" => api::call_api(&rt, move || async move { a.set_subscription_enabled(&i, false).await.map(|_| ()) }).await,
                _ => api::call_api(&rt, move || async move { a.delete_subscription(&i).await.map(|_| ()) }).await,
            };
            let _ = this.update(cx, |this, cx| {
                match r { Ok(_) => { this.toast(cx, ToastKind::Success, "操作完成"); this.refresh(cx); } Err(e) => this.toast(cx, ToastKind::Error, e.message()) }
            });
        }).detach();
    }

    fn open_runs(&mut self, id: &str, cx: &mut Context<Self>) {
        let id = id.to_string();
        let b = cx.global::<Backend>();
        let (rt, api) = (b.rt.clone(), b.api.clone());
        let wh = self.window_handle;
        cx.spawn(async move |_, cx| {
            let (a, i) = (api.clone(), id.clone());
            let r = api::call_api(&rt, move || async move { a.list_runs(&i, 1).await }).await;
            if let Ok(runs) = r {
                let _ = wh.update(cx, |_, window, cx| {
                    window.open_dialog(cx, move |d: Dialog, _, _| {
                        let mut list = v_flex().max_h(px(400.)).overflow_y_scrollbar();
                        if runs.is_empty() {
                            list = list.child(div().py_4().text_center().child("暂无运行记录"));
                        }
                        for run in &runs {
                            list = list.child(run_row(run));
                        }
                        d.title("运行历史").width(px(600.)).child(list)
                    });
                });
            }
        }).detach();
    }
}

fn run_row(run: &RunRecord) -> impl IntoElement {
    let v = serde_json::to_value(run).unwrap_or_default();
    let status = v.get("status").and_then(|s| s.as_str()).unwrap_or("").to_string();
    let started = v.get("started_at").and_then(|s| s.as_str()).unwrap_or("").to_string();
    let added = v.get("added_count").and_then(|s| s.as_u64()).unwrap_or(0);
    let failed = v.get("failed_count").and_then(|s| s.as_u64()).unwrap_or(0);
    h_flex().gap_2().items_center().px_2().py_1p5().border_b_1()
        .child(div().flex_1().child(started))
        .child(Tag::new().small().child(status))
        .child(div().child(format!("新增 {added} / 失败 {failed}")))
}

impl Render for ShareSyncView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let toolbar = h_flex().gap_2().items_center().px_4().py_2().border_b_1().border_color(theme.border)
            .child(div().flex_1().child(format!("分享同步 · {} 个订阅", self.subs.len())))
            .child(Button::new("ss-new").primary().small().icon(IconName::Plus).label("新建").on_click(cx.listener(|this, _, _, cx| this.open_create(cx))))
            .child(Button::new("ss-refresh").small().icon(IconName::RefreshCw).on_click(cx.listener(|this, _, _, cx| this.refresh(cx))));

        let mut list = v_flex().gap_2().p_4().overflow_y_scrollbar();
        if self.subs.is_empty() {
            list = list.child(div().py_8().text_center().text_color(theme.muted_foreground).child("暂无分享订阅"));
        }
        for s in &self.subs {
            let id = s.id.clone();
            let enabled = s.enabled;
            let link_invalid = s.consecutive_link_failures > 0;
            let id_trigger = id.clone();
            let id_resume = id.clone();
            let id_toggle = id.clone();
            let id_runs = id.clone();
            let id_del = id.clone();
            list = list.child(
                v_flex().gap_2().p_3().border_1().border_color(theme.border).rounded_lg()
                    .child(h_flex().gap_2().items_center()
                        .child(Icon::new(IconName::FolderSync).size(px(18.)).text_color(theme.primary))
                        .child(div().flex_1().child(s.name.clone()))
                        .child(Tag::new().small().child(if enabled { "已启用" } else { "已停用" }))
                        .when(link_invalid, |this| this.child(Tag::new().small().child("链接失效"))))
                    .child(div().text_color(theme.muted_foreground).truncate().child(s.share_url.clone()))
                    .child(h_flex().gap_2()
                        .child(Button::new(format!("ss-trigger-{id}")).small().label("立即同步").on_click(cx.listener(move |this, _, _, cx| this.act(&id_trigger, "trigger", cx))))
                        .when(link_invalid, |this| this.child(Button::new(format!("ss-resume-{id}")).small().label("恢复").on_click(cx.listener(move |this, _, _, cx| this.act(&id_resume, "resume", cx)))))
                        .child(Button::new(format!("ss-toggle-{id}")).small().label(if enabled { "停用" } else { "启用" }).on_click(cx.listener(move |this, _, _, cx| this.act(&id_toggle, if enabled { "disable" } else { "enable" }, cx))))
                        .child(Button::new(format!("ss-runs-{id}")).small().label("历史").on_click(cx.listener(move |this, _, _, cx| this.open_runs(&id_runs, cx))))
                        .child(Button::new(format!("ss-del-{id}")).danger().small().label("删除").on_click(cx.listener(move |this, _, _, cx| this.act(&id_del, "delete", cx))))),
            );
        }
        v_flex().flex_1().overflow_hidden().child(toolbar).child(div().flex_1().child(list))
    }
}
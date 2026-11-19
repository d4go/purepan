//! 分享同步页:总览头部 + 三段式订阅卡片(同步图标、状态胶囊、链接与目标路径、操作),
//! 新建订阅 / 立即同步 / 恢复 / 启停 / 运行历史 / 删除。

use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::dialog::Dialog;
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::tag::Tag;
use gpui_kit::component::{Icon, Sizable as _, WindowExt as _, h_flex, v_flex};
use gpui_kit::{
    AnyElement, AnyWindowHandle, AppContext as _, Context, Entity, InteractiveElement as _,
    IntoElement, ParentElement as _, Render, Styled as _, Subscription, Window, div, px,
};
use serde_json::json;

use purepan_backend::share_sync::config::SyncTarget;

use crate::api::share_sync::{RunRecord, ShareSubscription};
use crate::api::{self};
use crate::bridge::{EventHub, ToastKind, UiEvent};
use crate::components::task_ui::{
    circle_button, empty_state, sep_dot, soft_pill, task_card,
};
use crate::components::toast;
use crate::design::{self, Palette};
use crate::state::Backend;
use crate::util;

fn target_paths(s: &ShareSubscription) -> String {
    let mut parts: Vec<String> = Vec::new();
    for t in &s.targets {
        match t {
            SyncTarget::Netdisk(t) => parts.push(t.remote_path.clone()),
            SyncTarget::Local(t) => parts.push(t.local_path.display().to_string()),
        }
    }
    parts.join("、")
}

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

    fn open_create(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let view = cx.entity().clone();
        let name = self.name_input.clone();
        let url = self.url_input.clone();
        let pwd = self.pwd_input.clone();
        let path = self.path_input.clone();
        let v2 = view.clone();
        window.open_dialog(cx, move |d: Dialog, _, _| {
            let v3 = v2.clone();
            d.title("新建分享订阅").width(px(520.))
                .child(v_flex().gap_3()
                    .child(h_flex().gap_2().items_center().child("名称").child(Input::new(&name).large()))
                    .child(h_flex().gap_2().items_center().child("分享链接").child(Input::new(&url).large()))
                    .child(h_flex().gap_2().items_center().child("提取码").child(Input::new(&pwd).large()))
                    .child(h_flex().gap_2().items_center().child("网盘目录").child(Input::new(&path).large())))
                .footer(h_flex().gap_2().justify_end()
                    .child(Button::new("ss-cancel").label("取消").on_click(|_, w, cx| w.close_dialog(cx)))
                    .child(Button::new("ss-create").primary().label("创建").on_click(move |_, window, cx| { v3.update(cx, |this, cx| this.create(window, cx)); })))
        });
    }

    fn create(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let name = self.name_input.read(cx).value().trim().to_string();
        let url = self.url_input.read(cx).value().trim().to_string();
        let pwd = self.pwd_input.read(cx).value().trim().to_string();
        let path = self.path_input.read(cx).value().trim().to_string();
        if name.is_empty() || url.is_empty() { toast::show(window, cx, ToastKind::Warning, "请填写名称和分享链接"); return; }
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
                    window.open_dialog(cx, move |d: Dialog, _, cx| {
                        let palette = design::palette(cx);
                        let mut list = v_flex().max_h(px(400.)).overflow_y_scrollbar();
                        if runs.is_empty() {
                            list = list.child(div().py_4().text_center().text_color(palette.muted).child("暂无运行记录"));
                        }
                        for run in &runs {
                            list = list.child(run_row(run, &palette));
                        }
                        d.title("运行历史").width(px(600.)).child(list)
                    });
                });
            }
        }).detach();
    }

    fn render_row(&self, s: &ShareSubscription, p: &Palette, cx: &mut Context<Self>) -> AnyElement {
        let id = s.id.clone();
        let enabled = s.enabled;
        let link_invalid = s.link_invalid || s.consecutive_link_failures > 0;
        let paths = target_paths(s);

        // 左:同步图标
        let tile = div()
            .size(px(44.))
            .rounded(px(12.))
            .bg(p.primary_soft)
            .flex()
            .items_center()
            .justify_center()
            .flex_shrink_0()
            .child(Icon::new(IconName::FolderSync).size(px(20.)).text_color(p.primary))
            .into_any_element();

        // 中:名称 / 状态 / 链接与目标
        let mut title = h_flex().gap(px(8.)).items_center().min_w_0().overflow_hidden();
        title = title.child(
            div()
                .flex_1()
                .min_w_0()
                .truncate()
                .text_size(px(14.))
                .font_weight(gpui_kit::FontWeight::MEDIUM)
                .child(s.name.clone()),
        );
        title = title.child(if enabled {
            soft_pill("已启用", if p.is_dark { p.primary.alpha(0.16) } else { gpui_kit::rgb(0xD1FAE5).into() }, if p.is_dark { gpui_kit::rgb(0x34D399).into() } else { gpui_kit::rgb(0x059669).into() })
        } else {
            soft_pill("已停用", p.selected_bg, p.muted)
        });
        if link_invalid {
            title = title.child(soft_pill("链接失效", if p.is_dark { p.danger.alpha(0.16) } else { gpui_kit::rgb(0xFEE2E2).into() }, p.danger));
        }

        let mut mid = v_flex().flex_1().min_w_0().gap(px(6.)).overflow_hidden().child(title);

        let mut meta = h_flex()
            .gap(px(6.))
            .items_center()
            .min_w_0()
            .overflow_hidden()
            .text_size(px(12.))
            .text_color(p.faint);
        meta = meta.child(div().truncate().child(s.share_url.clone()));
        if !paths.is_empty() {
            meta = meta.child(sep_dot(p));
            meta = meta.child(div().truncate().child(format!("同步至 {paths}")));
        }
        mid = mid.child(meta);

        // 右:同步 / 恢复 / 启停 / 历史 / 删除
        let mut actions = h_flex().gap(px(2.)).items_center().flex_shrink_0();
        let call = |key: &str| gpui_kit::SharedString::from(format!("{key}:{id}"));
        let id_trigger = id.clone();
        actions = actions.child(circle_button(p, call("ss-trigger"), IconName::RefreshCw, move |this, _, cx| {
            this.act(&id_trigger, "trigger", cx)
        }, cx));
        if link_invalid {
            let id_resume = id.clone();
            actions = actions.child(circle_button(p, call("ss-resume"), IconName::RotateCw, move |this, _, cx| {
                this.act(&id_resume, "resume", cx)
            }, cx));
        }
        let id_toggle = id.clone();
        let next_op = if enabled { "disable" } else { "enable" };
        actions = actions.child(circle_button(p, call("ss-toggle"), if enabled { IconName::CirclePause } else { IconName::CirclePlay }, move |this, _, cx| {
            this.act(&id_toggle, next_op, cx)
        }, cx));
        let id_runs = id.clone();
        actions = actions.child(circle_button(p, call("ss-runs"), IconName::ListTree, move |this, _, cx| {
            this.open_runs(&id_runs, cx)
        }, cx));
        let id_del = id.clone();
        actions = actions.child(circle_button(p, call("ss-del"), IconName::Trash, move |this, _, cx| {
            this.act(&id_del, "delete", cx)
        }, cx));

        task_card(p, tile, mid.into_any_element(), actions.into_any_element())
    }
}

fn run_row(run: &RunRecord, p: &Palette) -> AnyElement {
    let v = serde_json::to_value(run).unwrap_or_default();
    let status = v.get("status").and_then(|s| s.as_str()).unwrap_or("").to_string();
    let started = v.get("started_at").and_then(|s| s.as_i64()).unwrap_or(0);
    let added = v.get("added_count").and_then(|s| s.as_u64()).unwrap_or(0);
    let failed = v.get("failed_count").and_then(|s| s.as_u64()).unwrap_or(0);
    let started_text = if started > 0 { util::format_timestamp(started) } else { "-".into() };
    h_flex()
        .gap(px(8.))
        .items_center()
        .px(px(10.))
        .py(px(8.))
        .border_b_1()
        .border_color(p.border)
        .child(div().flex_1().truncate().text_size(px(12.)).child(started_text))
        .child(crate::components::task_ui::status_tag(&status, p))
        .child(div().text_size(px(12.)).text_color(p.faint).child(format!("新增 {added} / 失败 {failed}")))
        .into_any_element()
}

impl Render for ShareSyncView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = design::palette(cx);
        let enabled_count = self.subs.iter().filter(|s| s.enabled).count();
        let invalid_count = self.subs.iter().filter(|s| s.link_invalid || s.consecutive_link_failures > 0).count();

        let status_pill = if enabled_count > 0 {
            soft_pill(format!("{enabled_count} 个已启用"), p.primary_soft, p.primary)
        } else if !self.subs.is_empty() {
            soft_pill(format!("{} 个订阅", self.subs.len()), p.primary_soft, p.primary)
        } else {
            Tag::new().small().rounded_full().child("暂无订阅")
        };

        let mut header_right = h_flex().gap(px(6.)).items_center();
        if invalid_count > 0 {
            header_right = header_right.child(soft_pill(format!("{invalid_count} 个链接失效"), if p.is_dark { p.danger.alpha(0.16) } else { gpui_kit::rgb(0xFEE2E2).into() }, p.danger));
        }
        header_right = header_right
            .child(
                Button::new("ss-new")
                    .primary()
                    .small()
                    .icon(IconName::Plus)
                    .label("新建")
                    .on_click(cx.listener(|this, _, window, cx| this.open_create(window, cx))),
            )
            .child(
                Button::new("ss-refresh")
                    .ghost()
                    .small()
                    .icon(IconName::RefreshCw)
                    .on_click(cx.listener(|this, _, _, cx| this.refresh(cx))),
            );

        let header = h_flex()
            .h(px(68.))
            .px(px(32.))
            .gap(px(12.))
            .items_center()
            .flex_shrink_0()
            .child(div().text_size(px(18.)).font_weight(gpui_kit::FontWeight::SEMIBOLD).child("分享同步"))
            .child(status_pill)
            .child(div().flex_1())
            .child(header_right)
            .into_any_element();

        let body: AnyElement = if self.subs.is_empty() {
            empty_state(
                &p,
                IconName::FolderSync,
                "暂无分享订阅",
                "订阅他人分享链接,文件更新后自动同步到你的网盘",
                Some(
                    Button::new("ss-empty-new")
                        .small()
                        .icon(IconName::Plus)
                        .label("新建订阅")
                        .on_click(cx.listener(|this, _, window, cx| this.open_create(window, cx)))
                        .into_any_element(),
                ),
            )
        } else {
            let mut list = v_flex()
                .id("ss-scroll")
                .flex_1()
                .min_h_0()
                .gap(px(10.))
                .px(px(32.))
                .py(px(16.))
                .overflow_y_scrollbar();
            for s in &self.subs {
                list = list.child(self.render_row(s, &p, cx));
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
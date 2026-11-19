//! 分享对话框(移植 ShareDialog.vue):选有效期 + 提取码(可随机)→ 创建分享 → 展示链接/提取码并复制。

use std::sync::Arc;

use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::radio::RadioGroup;
use gpui_kit::component::{ActiveTheme as _, Sizable as _, WindowExt as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AnyWindowHandle, App, AppContext as _, ClipboardItem, Context, Entity, IntoElement,
    ParentElement as _, Render, Styled as _, Window, div, px,
};

use crate::api::{self, ApiClient};
use crate::bridge::ToastKind;
use crate::components::toast;
use crate::state::Backend;

pub struct ShareDialog {
    paths: Vec<String>,
    period: usize, // index into [0,1,7,30]
    pwd: Entity<InputState>,
    result: Option<(String, String)>, // (link, pwd)
    creating: bool,
    rt: tokio::runtime::Handle,
    api: Arc<ApiClient>,
    window_handle: AnyWindowHandle,
}

const PERIODS: [i32; 4] = [0, 1, 7, 30];
const PERIOD_LABELS: [&str; 4] = ["永久", "1 天", "7 天", "30 天"];

impl ShareDialog {
    pub fn new(paths: Vec<String>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let b = cx.global::<Backend>();
        let (rt, api) = (b.rt.clone(), b.api.clone());
        let pwd = cx.new(|cx| InputState::new(window, cx).placeholder("提取码(4 位,留空则无)"));
        Self { paths, period: 0, pwd, result: None, creating: false, rt, api, window_handle: window.window_handle() }
    }

    fn toast(&self, cx: &mut Context<Self>, kind: ToastKind, msg: impl Into<String>) {
        let msg = msg.into();
        let wh = self.window_handle;
        let _ = wh.update(cx, |_, w, cx| toast::show(w, cx, kind, msg.clone()));
    }

    fn random_pwd(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        use rand::Rng;
        const CHARS: &[u8] = b"abcdefghijklmnopqrstuvwxyz0123456789";
        let mut rng = rand::thread_rng();
        let pwd: String = (0..4).map(|_| CHARS[rng.gen_range(0..CHARS.len())] as char).collect();
        self.pwd.update(cx, |s, cx| s.set_value(pwd, window, cx));
    }

    fn create(&mut self, cx: &mut Context<Self>) {
        let pwd = self.pwd.read(cx).value().trim().to_string();
        let pwd = if pwd.is_empty() { None } else { Some(pwd) };
        let period = PERIODS[self.period];
        let paths = self.paths.clone();
        self.creating = true;
        cx.notify();
        let (rt, api) = (self.rt.clone(), self.api.clone());
        cx.spawn(async move |this, cx| {
            let (a, p, pw) = (api.clone(), paths.clone(), pwd.clone());
            let r = api::call_api(&rt, move || async move { a.create_share(&p, period, pw.as_deref()).await }).await;
            let _ = this.update(cx, |this, cx| {
                this.creating = false;
                match r {
                    Ok(res) => this.result = Some((res.link, res.pwd)),
                    Err(e) => this.toast(cx, ToastKind::Error, e.message()),
                }
                cx.notify();
            });
        }).detach();
    }

    fn copy(&self, text: String, window: &mut Window, cx: &mut Context<Self>) {
        cx.write_to_clipboard(ClipboardItem::new_string(text));
        toast::show(window, cx, ToastKind::Success, "已复制");
    }
}

impl Render for ShareDialog {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let mut form = v_flex().gap_3().w(px(480.));
        if let Some((link, pwd)) = self.result.clone() {
            let link2 = link.clone();
            let both = if pwd.is_empty() { link.clone() } else { format!("{link} 提取码: {pwd}") };
            form = form
                .child(div().child("分享成功"))
                .child(div().px_2().py_1().bg(theme.secondary).rounded_md().child(link.clone()))
                .when(!pwd.is_empty(), |this| this.child(div().child(format!("提取码: {pwd}"))))
                .child(h_flex().gap_2().justify_end()
                    .child(Button::new("sh-copy-link").label("复制链接").on_click(cx.listener(move |this, _, window, cx| this.copy(link2.clone(), window, cx))))
                    .child(Button::new("sh-copy-both").primary().label("复制全部").on_click(cx.listener(move |this, _, window, cx| this.copy(both.clone(), window, cx))))
                    .child(Button::new("sh-close").label("关闭").on_click(|_, w, cx| w.close_dialog(cx))));
            return form;
        }
        form = form
            .child(div().text_color(theme.muted_foreground).child(format!("分享 {} 项", self.paths.len())))
            .child(h_flex().gap_2().items_center().child("有效期").child(
                RadioGroup::new("sh-period").children(PERIOD_LABELS)
                    .selected_index(Some(self.period))
                    .on_change(cx.listener(|this, idx, _, cx| { this.period = *idx; cx.notify(); })),
            ))
            .child(h_flex().gap_2().items_center().child("提取码").child(Input::new(&self.pwd).large())
                .child(Button::new("sh-random").small().label("随机").on_click(cx.listener(|this, _, window, cx| this.random_pwd(window, cx)))))
            .child(h_flex().gap_2().justify_end()
                .child(Button::new("sh-cancel").label("取消").on_click(|_, w, cx| w.close_dialog(cx)))
                .child(Button::new("sh-create").primary().loading(self.creating).label("创建分享").on_click(cx.listener(|this, _, _, cx| this.create(cx)))));
        form
    }
}
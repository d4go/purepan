//! 系统设置页(移植 SettingsView.vue 核心小节):账号管理、下载/上传、加密、代理、界面、关于。

use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::radio::RadioGroup;
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::switch::Switch;
use gpui_kit::component::tag::Tag;
use gpui_kit::component::{ActiveTheme as _, Icon, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AnyWindowHandle, App, AppContext as _, Context, Entity, IntoElement, ParentElement as _,
    Render, Styled as _, Subscription, Window, div, px,
};
use serde_json::{Value, json};

use crate::api::autobackup::EncryptionStatusResponse;
use crate::api::dto::AccountSummary;
use crate::api::{self};
use crate::app::AppController;
use crate::bridge::{EventHub, ToastKind, UiEvent};
use crate::components::toast;
use crate::state::{AuthStore, Backend};

pub struct SettingsView {
    controller: Entity<AppController>,
    config: Value,
    accounts: Vec<AccountSummary>,
    enc: Option<EncryptionStatusResponse>,
    proxy_status: String,
    // 编辑态
    download_dir: Entity<InputState>,
    dl_threads: Entity<InputState>,
    dl_concurrent: Entity<InputState>,
    dl_retries: Entity<InputState>,
    ul_threads: Entity<InputState>,
    ul_concurrent: Entity<InputState>,
    ul_retries: Entity<InputState>,
    ask_each_time: bool,
    skip_hidden: bool,
    close_behavior: usize, // 0=close,1=minimize_to_tray
    saving: bool,
    window_handle: AnyWindowHandle,
    _subscriptions: Vec<Subscription>,
}

fn section_title(title: &str, cx: &App) -> impl IntoElement {
    div()
        .text_size(px(12.))
        .font_weight(gpui_kit::FontWeight::SEMIBOLD)
        .text_color(cx.theme().muted_foreground)
        .child(title.to_string())
}

fn field_label(text: &str, cx: &App) -> impl IntoElement {
    div().w(px(88.)).text_color(cx.theme().muted_foreground).child(text.to_string())
}

fn num_field(label: &str, input: &Entity<InputState>, cx: &App) -> impl IntoElement {
    h_flex()
        .gap_1p5()
        .items_center()
        .child(div().text_color(cx.theme().muted_foreground).text_size(px(12.)).child(label.to_string()))
        .child(Input::new(input).small().w(px(84.)))
}

impl SettingsView {
    pub fn new(controller: Entity<AppController>, hub: Entity<EventHub>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let mut mk = |cx: &mut Context<Self>, ph: &str| cx.new(|cx| InputState::new(window, cx).placeholder(ph));
        let download_dir = mk(cx, "下载目录");
        let dl_threads = mk(cx, "下载线程");
        let dl_concurrent = mk(cx, "并发任务");
        let dl_retries = mk(cx, "重试次数");
        let ul_threads = mk(cx, "上传线程");
        let ul_concurrent = mk(cx, "并发任务");
        let ul_retries = mk(cx, "重试次数");
        let sub = cx.subscribe(&hub, |this, _, ev, cx| {
            if matches!(ev, UiEvent::AccountChanged { .. }) { this.load_accounts(cx); }
        });
        let mut v = Self {
            controller, config: Value::Null, accounts: Vec::new(), enc: None, proxy_status: String::new(),
            download_dir, dl_threads, dl_concurrent, dl_retries, ul_threads, ul_concurrent, ul_retries,
            ask_each_time: false, skip_hidden: false, close_behavior: 0, saving: false,
            window_handle: window.window_handle(), _subscriptions: vec![sub],
        };
        v.load_all(cx);
        v
    }

    fn toast(&self, cx: &mut Context<Self>, kind: ToastKind, msg: impl Into<String>) {
        let msg = msg.into();
        let wh = self.window_handle;
        let _ = wh.update(cx, |_, w, cx| toast::show(w, cx, kind, msg.clone()));
    }

    fn load_all(&mut self, cx: &mut Context<Self>) {
        self.load_accounts(cx);
        let b = cx.global::<Backend>();
        let (rt, api) = (b.rt.clone(), b.api.clone());
        cx.spawn(async move |this, cx| {
            let a = api.clone();
            let cfg = api::call_api(&rt, move || async move { a.get_config().await }).await;
            let a2 = api.clone();
            let enc = api::call_api(&rt, move || async move { a2.encryption_status().await }).await.ok();
            let a3 = api.clone();
            let proxy = api::call_api(&rt, move || async move { a3.get_proxy_status().await }).await.ok();
            let _ = this.update(cx, |this, cx| {
                if let Ok(c) = cfg {
                    this.config = serde_json::to_value(&c).unwrap_or(Value::Null);
                    this.populate_inputs(cx);
                }
                this.enc = enc;
                if let Some(p) = proxy {
                    this.proxy_status = p.status.as_str().unwrap_or("").to_string();
                }
                cx.notify();
            });
        }).detach();
    }

    fn load_accounts(&mut self, cx: &mut Context<Self>) {
        let b = cx.global::<Backend>();
        let (rt, api) = (b.rt.clone(), b.api.clone());
        cx.spawn(async move |this, cx| {
            let a = api.clone();
            let r = api::call_api(&rt, move || async move { a.list_accounts().await }).await;
            let _ = this.update(cx, |this, cx| {
                if let Ok(resp) = r { this.accounts = resp.accounts; }
                cx.notify();
            });
        }).detach();
    }

    fn populate_inputs(&mut self, cx: &mut Context<Self>) {
        let g = |path: &[&str]| -> String {
            let mut v = &self.config;
            for k in path { v = v.get(k).unwrap_or(&Value::Null); }
            match v { Value::String(s) => s.clone(), Value::Number(n) => n.to_string(), Value::Bool(b) => b.to_string(), _ => String::new() }
        };
        self.ask_each_time = self.config["download"]["ask_each_time"].as_bool().unwrap_or(false);
        self.skip_hidden = self.config["upload"]["skip_hidden_files"].as_bool().unwrap_or(false);
        self.close_behavior = if self.config["ui"]["close_behavior"].as_str() == Some("minimize_to_tray") { 1 } else { 0 };
        let vals = [
            (self.download_dir.clone(), g(&["download", "download_dir"])),
            (self.dl_threads.clone(), g(&["download", "max_global_threads"])),
            (self.dl_concurrent.clone(), g(&["download", "max_concurrent_tasks"])),
            (self.dl_retries.clone(), g(&["download", "max_retries"])),
            (self.ul_threads.clone(), g(&["upload", "max_global_threads"])),
            (self.ul_concurrent.clone(), g(&["upload", "max_concurrent_tasks"])),
            (self.ul_retries.clone(), g(&["upload", "max_retries"])),
        ];
        let wh = self.window_handle;
        let _ = wh.update(cx, move |_, window, cx| {
            for (input, val) in vals {
                input.update(cx, |s, cx| s.set_value(val.clone(), window, cx));
            }
        });
    }

    fn save(&mut self, cx: &mut Context<Self>) {
        let rd = |e: &Entity<InputState>| e.read(cx).value().trim().to_string();
        let mut cfg = self.config.clone();
        cfg["download"]["download_dir"] = json!(rd(&self.download_dir));
        cfg["download"]["ask_each_time"] = json!(self.ask_each_time);
        if let Ok(n) = rd(&self.dl_threads).parse::<u64>() { cfg["download"]["max_global_threads"] = json!(n); }
        if let Ok(n) = rd(&self.dl_concurrent).parse::<u64>() { cfg["download"]["max_concurrent_tasks"] = json!(n); }
        if let Ok(n) = rd(&self.dl_retries).parse::<u64>() { cfg["download"]["max_retries"] = json!(n); }
        if let Ok(n) = rd(&self.ul_threads).parse::<u64>() { cfg["upload"]["max_global_threads"] = json!(n); }
        if let Ok(n) = rd(&self.ul_concurrent).parse::<u64>() { cfg["upload"]["max_concurrent_tasks"] = json!(n); }
        if let Ok(n) = rd(&self.ul_retries).parse::<u64>() { cfg["upload"]["max_retries"] = json!(n); }
        cfg["upload"]["skip_hidden_files"] = json!(self.skip_hidden);
        cfg["ui"]["close_behavior"] = json!(if self.close_behavior == 1 { "minimize_to_tray" } else { "close" });
        self.saving = true;
        cx.notify();
        let b = cx.global::<Backend>();
        let (rt, api) = (b.rt.clone(), b.api.clone());
        cx.spawn(async move |this, cx| {
            let (a, c) = (api.clone(), cfg.clone());
            let r = api::call_api(&rt, move || async move { a.update_config_value(c).await }).await;
            let _ = this.update(cx, |this, cx| {
                this.saving = false;
                match r {
                    Ok(_) => { this.toast(cx, ToastKind::Success, "设置已保存"); this.load_all(cx); }
                    Err(e) => this.toast(cx, ToastKind::Error, e.message()),
                }
            });
        }).detach();
    }

    fn switch_account(&mut self, uid: u64, cx: &mut Context<Self>) {
        let b = cx.global::<Backend>();
        let (rt, api) = (b.rt.clone(), b.api.clone());
        cx.spawn(async move |this, cx| {
            let a = api.clone();
            let r = api::call_api(&rt, move || async move { a.switch_account(uid).await }).await;
            let _ = this.update(cx, |this, cx| { if let Err(e) = r { this.toast(cx, ToastKind::Error, e.message()); } else { this.load_accounts(cx); } });
        }).detach();
    }

    fn delete_account(&mut self, uid: u64, cx: &mut Context<Self>) {
        let b = cx.global::<Backend>();
        let (rt, api) = (b.rt.clone(), b.api.clone());
        cx.spawn(async move |this, cx| {
            let a = api.clone();
            let r = api::call_api(&rt, move || async move { a.delete_account(uid, true).await }).await;
            let _ = this.update(cx, |this, cx| { if let Err(e) = r { this.toast(cx, ToastKind::Error, e.message()); } else { this.load_accounts(cx); } });
        }).detach();
    }

    fn gen_key(&mut self, cx: &mut Context<Self>) {
        let b = cx.global::<Backend>();
        let (rt, api) = (b.rt.clone(), b.api.clone());
        cx.spawn(async move |this, cx| {
            let a = api.clone();
            let r = api::call_api(&rt, move || async move { a.generate_encryption_key(Some("aes-256-gcm")).await }).await;
            let _ = this.update(cx, |this, cx| {
                match r { Ok(_) => { this.toast(cx, ToastKind::Success, "密钥已生成"); this.load_all(cx); } Err(e) => this.toast(cx, ToastKind::Error, e.message()) }
            });
        }).detach();
    }

    fn export_key(&mut self, cx: &mut Context<Self>) {
        let b = cx.global::<Backend>();
        let (rt, api) = (b.rt.clone(), b.api.clone());
        let wh = self.window_handle;
        cx.spawn(async move |this, cx| {
            let a = api.clone();
            let r = api::call_api(&rt, move || async move { a.export_encryption_key().await }).await;
            let _ = this.update(cx, |this, cx| {
                match r {
                    Ok(key) => { let _ = wh.update(cx, |_, w, cx| { cx.write_to_clipboard(gpui_kit::ClipboardItem::new_string(key.clone())); }); this.toast(cx, ToastKind::Success, "密钥已复制到剪贴板"); }
                    Err(e) => this.toast(cx, ToastKind::Error, e.message()),
                }
            });
        }).detach();
    }

    fn delete_key(&mut self, cx: &mut Context<Self>) {
        let b = cx.global::<Backend>();
        let (rt, api) = (b.rt.clone(), b.api.clone());
        cx.spawn(async move |this, cx| {
            let a = api.clone();
            let r = api::call_api(&rt, move || async move { a.delete_encryption_key(true).await }).await;
            let _ = this.update(cx, |this, cx| { if let Err(e) = r { this.toast(cx, ToastKind::Error, e.message()); } else { this.toast(cx, ToastKind::Success, "密钥已删除"); this.load_all(cx); } });
        }).detach();
    }

    fn test_proxy(&mut self, cx: &mut Context<Self>) {
        let proxy: purepan_backend::common::ProxyConfig = serde_json::from_value(self.config["network"]["proxy"].clone()).unwrap_or_default();
        let b = cx.global::<Backend>();
        let (rt, api) = (b.rt.clone(), b.api.clone());
        cx.spawn(async move |this, cx| {
            let (a, p) = (api.clone(), proxy.clone());
            let r = api::call_api(&rt, move || async move { a.test_proxy(&p).await }).await;
            let _ = this.update(cx, |this, cx| {
                match r {
                    Ok(resp) if resp.success => this.toast(cx, ToastKind::Success, format!("代理正常 ({}ms)", resp.latency_ms.unwrap_or(0))),
                    Ok(resp) => this.toast(cx, ToastKind::Error, resp.error.unwrap_or_else(|| "代理失败".into())),
                    Err(e) => this.toast(cx, ToastKind::Error, e.message()),
                }
            });
        }).detach();
    }
}

impl Render for SettingsView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        // 桌面端表单:内容限宽居中、统一控件尺寸,避免宽屏下拉满与高矮不齐
        let mut page = v_flex().w_full().max_w(px(820.)).gap_6().p_6();

        // 账号
        let mut acc = v_flex().gap_2().child(section_title("账号", cx));
        for a in &self.accounts {
            let uid = a.uid;
            let active = a.is_active;
            let name = a.nickname.clone().filter(|s| !s.is_empty()).unwrap_or_else(|| a.username.clone());
            let uid_switch = uid;
            let uid_del = uid;
            acc = acc.child(
                h_flex().gap_3().items_center().px_3().py_1p5().rounded_lg().bg(theme.secondary)
                    .child(div().flex_1().truncate().child(format!("{name}  ·  UID {uid}")))
                    .child(Tag::new().small().child(if active { "活跃" } else { "未激活" }))
                    .when(!active, |this| this.child(
                        Button::new(format!("set-switch-{uid}")).small().label("切换")
                            .on_click(cx.listener(move |this, _, _, cx| this.switch_account(uid_switch, cx)))))
                    .child(
                        Button::new(format!("set-del-{uid}")).ghost().small().icon(IconName::Trash)
                            .on_click(cx.listener(move |this, _, _, cx| this.delete_account(uid_del, cx)))),
            );
        }
        acc = acc.child(
            h_flex().child(
                Button::new("set-add-account").small().icon(IconName::Plus).label("添加账号")
                    .on_click(cx.listener(|this, _, _, cx| { this.controller.update(cx, |c, cx| c.add_account(cx)); })),
            ),
        );
        page = page.child(acc);

        // 下载
        let mut dl = v_flex().gap_3().child(section_title("下载", cx));
        dl = dl.child(h_flex().gap_3().items_center()
            .child(field_label("下载目录", cx))
            .child(div().flex_1().max_w(px(440.)).child(Input::new(&self.download_dir).small()))
            .child(Switch::new("set-ask").small().label("每次询问").checked(self.ask_each_time)
                .on_change(cx.listener(|this, c, _, cx| { this.ask_each_time = *c; cx.notify(); }))));
        dl = dl.child(h_flex().gap_3().items_center()
            .child(field_label("并发与重试", cx))
            .child(h_flex().gap_3().items_center()
                .child(num_field("线程", &self.dl_threads, cx))
                .child(num_field("并发", &self.dl_concurrent, cx))
                .child(num_field("重试", &self.dl_retries, cx))));
        page = page.child(dl);

        // 上传
        let mut ul = v_flex().gap_3().child(section_title("上传", cx));
        ul = ul.child(h_flex().gap_3().items_center()
            .child(field_label("并发与重试", cx))
            .child(h_flex().gap_3().items_center()
                .child(num_field("线程", &self.ul_threads, cx))
                .child(num_field("并发", &self.ul_concurrent, cx))
                .child(num_field("重试", &self.ul_retries, cx))));
        ul = ul.child(h_flex().gap_3().items_center()
            .child(field_label("文件过滤", cx))
            .child(Switch::new("set-hidden").small().label("跳过隐藏文件").checked(self.skip_hidden)
                .on_change(cx.listener(|this, c, _, cx| { this.skip_hidden = *c; cx.notify(); }))));
        page = page.child(ul);

        // 加密
        let enc_text = match &self.enc {
            Some(e) if e.has_key => format!("已启用 · {}", e.algorithm),
            _ => "未配置密钥".to_string(),
        };
        let mut enc = v_flex().gap_3().child(section_title("加密", cx));
        enc = enc.child(h_flex().gap_3().items_center()
            .child(field_label("密钥", cx))
            .child(div().flex_1().text_color(theme.muted_foreground).child(enc_text))
            .child(h_flex().gap_2()
                .child(Button::new("set-gen-key").small().label("生成").on_click(cx.listener(|this, _, _, cx| this.gen_key(cx))))
                .child(Button::new("set-export-key").small().label("导出").on_click(cx.listener(|this, _, _, cx| this.export_key(cx))))
                .child(Button::new("set-del-key").danger().small().label("删除").on_click(cx.listener(|this, _, _, cx| this.delete_key(cx))))));
        page = page.child(enc);

        // 代理
        let mut net = v_flex().gap_3().child(section_title("网络", cx));
        net = net.child(h_flex().gap_3().items_center()
            .child(field_label("代理", cx))
            .child(div().flex_1().text_color(theme.muted_foreground)
                .child(format!("运行状态: {}", if self.proxy_status.is_empty() { "未知" } else { &self.proxy_status })))
            .child(Button::new("set-test-proxy").small().label("测试连接").on_click(cx.listener(|this, _, _, cx| this.test_proxy(cx)))));
        page = page.child(net);

        // 界面
        let mut win = v_flex().gap_3().child(section_title("窗口", cx));
        win = win.child(h_flex().gap_3().items_center()
            .child(field_label("关闭按钮", cx))
            .child(RadioGroup::new("set-close").children(["退出程序", "最小化到托盘"]).selected_index(Some(self.close_behavior))
                .on_change(cx.listener(|this, i, _, cx| { this.close_behavior = *i; cx.notify(); }))));
        page = page.child(win);

        // 关于
        let mut about = v_flex().gap_3().child(section_title("关于", cx));
        about = about.child(h_flex().gap_3().items_center()
            .child(field_label("版本", cx))
            .child(div().text_color(theme.muted_foreground).child("PurePan v0.1.5 · Apache-2.0")));
        page = page.child(about);

        page = page.child(h_flex().justify_end().pt_2().child(
            Button::new("set-save").primary().label("保存设置").loading(self.saving)
                .on_click(cx.listener(|this, _, _, cx| this.save(cx))),
        ));

        v_flex().flex_1().overflow_hidden().child(
            div().flex_1().overflow_y_scrollbar()
                .child(div().flex().justify_center().w_full().child(page)),
        )
    }
}
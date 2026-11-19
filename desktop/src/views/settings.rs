//! 系统设置页:账号、下载/上传、安全与网络、窗口与关于。

use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;

use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::dialog::Dialog;
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::switch::Switch;
use gpui_kit::component::{ActiveTheme as _, Icon, Sizable as _, WindowExt as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AnyElement, AnyWindowHandle, App, AppContext as _, ClickEvent, Context, Div, Entity,
    Focusable as _, FontWeight, InteractiveElement as _, IntoElement, ObjectFit,
    ParentElement as _, Render, RenderImage, StatefulInteractiveElement as _, Styled as _,
    StyledImage as _, Subscription, Window, div, img, px,
};
use serde_json::{Value, json};

use crate::api::autobackup::EncryptionStatusResponse;
use crate::api::dto::AccountSummary;
use crate::api::{self};
use crate::app::AppController;
use crate::bridge::{EventHub, ToastKind, UiEvent};
use crate::components::dialogs::file_picker::{LocalFilePicker, PickerMode};
use crate::components::toast;
use crate::design::{self, Palette};
use crate::state::{Backend, ThemePref};
use crate::util;

pub struct SettingsView {
    controller: Entity<AppController>,
    config: Value,
    accounts: Vec<AccountSummary>,
    avatars: HashMap<u64, Arc<RenderImage>>,
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
    theme: usize,          // 0=light,1=dark,2=system
    saving: bool,
    dirty: bool,
    window_handle: AnyWindowHandle,
    _subscriptions: Vec<Subscription>,
}

fn card(p: &Palette) -> Div {
    v_flex()
        .w_full()
        .gap(px(16.))
        .p(px(20.))
        .rounded(px(14.))
        .bg(p.card)
        .border_1()
        .border_color(p.border)
}

fn card_title(title: &str, p: &Palette) -> Div {
    div()
        .text_size(px(15.))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(p.text)
        .child(title.to_string())
}

fn row_label(text: &str, p: &Palette) -> Div {
    div()
        .w(px(120.))
        .flex_shrink_0()
        .text_size(px(13.))
        .text_color(p.muted)
        .child(text.to_string())
}

fn divider(p: &Palette) -> Div {
    div().h(px(1.)).w_full().bg(p.border)
}

fn num_field(label: &str, input: &Entity<InputState>, p: &Palette) -> AnyElement {
    h_flex()
        .gap(px(8.))
        .items_center()
        .child(div().text_size(px(12.)).text_color(p.faint).child(label.to_string()))
        .child(Input::new(input).large().w(px(72.)))
        .into_any_element()
}

fn avatar(name: &str, p: &Palette) -> AnyElement {
    let initial = name
        .chars()
        .next()
        .map(|c| c.to_uppercase().to_string())
        .unwrap_or_else(|| "?".into());
    div()
        .size(px(36.))
        .flex_shrink_0()
        .rounded_full()
        .bg(p.primary_soft)
        .text_color(p.primary)
        .text_size(px(14.))
        .font_weight(FontWeight::SEMIBOLD)
        .flex()
        .items_center()
        .justify_center()
        .child(initial)
        .into_any_element()
}

fn seg_item<G>(
    label: &'static str,
    selected: bool,
    p: &Palette,
    handler: G,
) -> AnyElement
where
    G: Fn(&ClickEvent, &mut Window, &mut App) + 'static,
{
    h_flex()
        .id(label)
        .h(px(30.))
        .px(px(12.))
        .gap(px(6.))
        .rounded(px(8.))
        .cursor_pointer()
        .text_size(px(13.))
        .text_color(if selected { p.text } else { p.muted })
        .when(selected, |d| {
            d.bg(p.card).border_1().border_color(p.border)
        })
        .when(selected, |d| {
            d.child(Icon::new(IconName::Check).size(px(12.)))
        })
        .child(label)
        .on_click(handler)
        .into_any_element()
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
        // 输入框失焦 / 回车时自动保存(仅在用户实际编辑后)
        let mut subs = vec![sub];
        let inputs = [
            download_dir.clone(), dl_threads.clone(), dl_concurrent.clone(), dl_retries.clone(),
            ul_threads.clone(), ul_concurrent.clone(), ul_retries.clone(),
        ];
        for input in inputs {
            let input_for_cb = input.clone();
            subs.push(cx.subscribe_in(&input, window, move |this, _, ev, window, cx| match ev {
                InputEvent::Change => {
                    if input_for_cb.read(cx).focus_handle(cx).is_focused(window) {
                        this.dirty = true;
                    }
                }
                InputEvent::Blur | InputEvent::PressEnter { .. } => {
                    if this.dirty {
                        this.save(cx);
                    }
                }
                _ => {}
            }));
        }
        let mut v = Self {
            controller, config: Value::Null, accounts: Vec::new(), avatars: HashMap::new(),
            enc: None, proxy_status: String::new(),
            download_dir, dl_threads, dl_concurrent, dl_retries, ul_threads, ul_concurrent, ul_retries,
            ask_each_time: false, skip_hidden: false, close_behavior: 0, theme: 2, saving: false, dirty: false,
            window_handle: window.window_handle(), _subscriptions: subs,
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
                if let Ok(resp) = r { this.accounts = resp.accounts; this.load_avatars(cx); }
                cx.notify();
            });
        }).detach();
    }

    /// 后台拉取账号头像(复用后端代理),解码后按 uid 缓存。
    fn load_avatars(&mut self, cx: &mut Context<Self>) {
        let b = cx.global::<Backend>();
        let (rt, api) = (b.rt.clone(), b.api.clone());
        let targets: Vec<(u64, String)> = self
            .accounts
            .iter()
            .filter_map(|a| a.avatar_url.clone().map(|u| (a.uid, u)))
            .filter(|(uid, _)| !self.avatars.contains_key(uid))
            .collect();
        for (uid, url) in targets {
            let (rt, api) = (rt.clone(), api.clone());
            cx.spawn(async move |this, cx| {
                let a = api.clone();
                let r = api::call_api(&rt, move || async move { a.fetch_avatar(&url).await }).await;
                if let Ok(bytes) = r {
                    if let Some(image) = util::decode_image_bytes(&bytes) {
                        let _ = this.update(cx, |this, cx| {
                            this.avatars.insert(uid, image);
                            cx.notify();
                        });
                    }
                }
            })
            .detach();
        }
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
        self.theme = match self.config["ui"]["theme"].as_str() {
            Some("light") => 0,
            Some("dark") => 1,
            _ => 2,
        };
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
        cfg["ui"]["theme"] = json!(match self.theme { 0 => "light", 1 => "dark", _ => "system" });
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
                    Ok(_) => {
                        this.dirty = false;
                        this.toast(cx, ToastKind::Success, "设置已保存");
                    }
                    Err(e) => this.toast(cx, ToastKind::Error, e.message()),
                }
            });
        }).detach();
    }

    fn apply_theme(&self, cx: &mut Context<Self>) {
        let pref = match self.theme {
            0 => ThemePref::Light,
            1 => ThemePref::Dark,
            _ => ThemePref::System,
        };
        crate::theme::apply_pref(pref, cx);
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
    fn account_avatar(&self, account: &AccountSummary, p: &Palette) -> AnyElement {
        if let Some(image) = self.avatars.get(&account.uid) {
            img(image.clone())
                .size(px(36.))
                .rounded_full()
                .object_fit(ObjectFit::Cover)
                .border_1()
                .border_color(p.border)
                .into_any_element()
        } else {
            let name = account
                .nickname
                .clone()
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| account.username.clone());
            avatar(&name, p)
        }
    }

    fn pick_download_dir(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let view = cx.entity().downgrade();
        let on_confirm: Rc<dyn Fn(Vec<String>, &mut Window, &mut App)> =
            Rc::new(move |paths, window, cx| {
                if let Some(dir) = paths.first().cloned() {
                    let _ = view.update(cx, move |v, cx| {
                        v.download_dir
                            .update(cx, |s, cx| s.set_value(dir.clone(), window, cx));
                        v.dirty = true;
                        v.save(cx);
                        cx.notify();
                    });
                }
            });
        let picker = cx.new(|cx| {
            LocalFilePicker::new(PickerMode::Directory, None, on_confirm, window, cx)
        });
        window.open_dialog(cx, move |d: Dialog, _, _| {
            d.title("选择下载目录").width(px(640.)).child(picker.clone())
        });
    }
}

impl Render for SettingsView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let p = design::palette(cx);

        let header = h_flex()
            .w_full()
            .items_center()
            .justify_between()
            .child(
                v_flex()
                    .gap(px(4.))
                    .child(
                        div()
                            .text_size(px(24.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(p.text)
                            .child("设置"),
                    )
                    .child(
                        div()
                            .text_size(px(12.))
                            .text_color(p.faint),
                    ),
            )
            .child(h_flex().when(self.saving, |d| {
                d.child(
                    div()
                        .text_size(px(12.))
                        .text_color(p.faint)
                        .child("保存中…"),
                )
            }));

        // 账号
        let mut account_card = card(&p).child(
            h_flex()
                .justify_between()
                .items_center()
                .child(card_title("账号", &p))
                .child(
                    Button::new("set-add-account")
                        .small()
                        .icon(IconName::Plus)
                        .label("添加账号")
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.controller.update(cx, |c, cx| c.add_account(cx));
                        })),
                ),
        );
        if self.accounts.is_empty() {
            account_card = account_card.child(
                div()
                    .text_size(px(13.))
                    .text_color(p.faint)
                    .child("尚未登录账号"),
            );
        }
        for a in &self.accounts {
            let uid = a.uid;
            let active = a.is_active;
            let name = a
                .nickname
                .clone()
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| a.username.clone());
            let uid_switch = uid;
            let uid_del = uid;
            account_card = account_card.child(
                h_flex()
                    .gap(px(12.))
                    .items_center()
                    .py(px(2.))
                    .child(self.account_avatar(a, &p))
                    .child(
                        h_flex()
                            .gap(px(8.))
                            .items_center()
                            .flex_1()
                            .min_w_0()
                            .child(
                                div()
                                    .text_size(px(14.))
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(p.text)
                                    .truncate()
                                    .child(name),
                            )
                            .child(
                                div()
                                    .text_size(px(12.))
                                    .text_color(p.faint)
                                    .child(format!("· UID {uid}")),
                            )
                            .when(active, |d| {
                                d.child(
                                    div()
                                        .px(px(8.))
                                        .py(px(2.))
                                        .rounded(px(9999.))
                                        .bg(theme.success.opacity(0.12))
                                        .text_color(theme.success)
                                        .text_size(px(11.))
                                        .child("活跃"),
                                )
                            }),
                    )
                    .when(!active, |d| {
                        d.child(
                            Button::new(format!("set-switch-{uid}"))
                                .small()
                                .label("切换")
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.switch_account(uid_switch, cx)
                                })),
                        )
                    })
                    .child(
                        Button::new(format!("set-del-{uid}"))
                            .ghost()
                            .small()
                            .icon(IconName::Trash)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.delete_account(uid_del, cx)
                            })),
                    ),
            );
        }

        // 下载与上传
        let mut dl_card = card(&p).child(card_title("下载与上传", &p));
        dl_card = dl_card.child(
            h_flex()
                .gap(px(12.))
                .items_center()
                .child(row_label("下载目录", &p))
                .child(div().flex_1().min_w_0().child(Input::new(&self.download_dir).large()))
                .child(
                    Button::new("set-pick-dir")
                        .small()
                        .label("选择")
                        .h(px(44.))
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.pick_download_dir(window, cx)
                        })),
                ),
        );
        dl_card = dl_card.child(
            h_flex()
                .gap(px(12.))
                .items_center()
                .child(row_label("每次下载前询问", &p))
                .child(
                    Switch::new("set-ask")
                        .checked(self.ask_each_time)
                        .on_change(cx.listener(|this, checked, _, cx| {
                            this.ask_each_time = *checked;
                            this.dirty = true;
                            this.save(cx);
                            cx.notify();
                        })),
                ),
        );
        dl_card = dl_card.child(
            h_flex()
                .gap(px(12.))
                .items_center()
                .child(row_label("下载并发", &p))
                .child(
                    h_flex()
                        .gap(px(24.))
                        .items_center()
                        .child(num_field("线程", &self.dl_threads, &p))
                        .child(num_field("并发", &self.dl_concurrent, &p))
                        .child(num_field("重试", &self.dl_retries, &p)),
                ),
        );
        dl_card = dl_card.child(divider(&p));
        dl_card = dl_card.child(
            h_flex()
                .gap(px(12.))
                .items_center()
                .child(row_label("上传并发", &p))
                .child(
                    h_flex()
                        .gap(px(24.))
                        .items_center()
                        .child(num_field("线程", &self.ul_threads, &p))
                        .child(num_field("并发", &self.ul_concurrent, &p))
                        .child(num_field("重试", &self.ul_retries, &p)),
                ),
        );
        dl_card = dl_card.child(
            h_flex()
                .gap(px(12.))
                .items_center()
                .child(row_label("跳过隐藏文件", &p))
                .child(
                    Switch::new("set-hidden")
                        .checked(self.skip_hidden)
                        .on_change(cx.listener(|this, checked, _, cx| {
                            this.skip_hidden = *checked;
                            this.dirty = true;
                            this.save(cx);
                            cx.notify();
                        })),
                ),
        );

        // 安全与网络
        let enc_text = match &self.enc {
            Some(e) if e.has_key => format!("已启用 · {}", e.algorithm),
            _ => "未配置密钥".to_string(),
        };
        let proxy_type = self.config["network"]["proxy"]["proxy_type"]
            .as_str()
            .unwrap_or("none");
        let proxy_text = if proxy_type == "none" {
            "直连（no_proxy）".to_string()
        } else {
            format!("代理（{proxy_type}）")
        };
        let mut sec_card = card(&p).child(card_title("安全与网络", &p));
        sec_card = sec_card.child(
            h_flex()
                .gap(px(12.))
                .items_center()
                .child(row_label("加密密钥", &p))
                .child(
                    div()
                        .flex_1()
                        .text_size(px(13.))
                        .text_color(p.muted)
                        .child(enc_text),
                )
                .child(
                    h_flex()
                        .gap(px(8.))
                        .child(
                            Button::new("set-gen-key")
                                .small()
                                .label("生成")
                                .on_click(cx.listener(|this, _, _, cx| this.gen_key(cx))),
                        )
                        .child(
                            Button::new("set-export-key")
                                .small()
                                .label("导出")
                                .on_click(cx.listener(|this, _, _, cx| this.export_key(cx))),
                        )
                        .child(
                            Button::new("set-del-key")
                                .small()
                                .label("删除")
                                .bg(theme.danger.opacity(0.10))
                                .text_color(theme.danger)
                                .on_click(cx.listener(|this, _, _, cx| this.delete_key(cx))),
                        ),
                ),
        );
        sec_card = sec_card.child(divider(&p));
        sec_card = sec_card.child(
            h_flex()
                .gap(px(12.))
                .items_center()
                .child(row_label("网络代理", &p))
                .child(
                    h_flex()
                        .gap(px(8.))
                        .items_center()
                        .flex_1()
                        .child(div().text_size(px(13.)).text_color(p.text).child(proxy_text))
                        .child(div().size(px(7.)).rounded_full().bg(theme.success)),
                )
                .child(
                    Button::new("set-test-proxy")
                        .small()
                        .label("测试连接")
                        .on_click(cx.listener(|this, _, _, cx| this.test_proxy(cx))),
                ),
        );

        // 窗口与关于
        let mut win_card = card(&p).child(card_title("窗口与关于", &p));
        win_card = win_card.child(
            h_flex()
                .gap(px(12.))
                .items_center()
                .child(row_label("外观主题", &p))
                .child(
                    h_flex()
                        .gap(px(2.))
                        .p(px(2.))
                        .rounded(px(10.))
                        .bg(theme.secondary)
                        .child(seg_item(
                            "浅色",
                            self.theme == 0,
                            &p,
                            cx.listener(|this, _, _, cx| {
                                this.theme = 0;
                                this.apply_theme(cx);
                                this.dirty = true;
                                this.save(cx);
                                cx.notify();
                            }),
                        ))
                        .child(seg_item(
                            "深色",
                            self.theme == 1,
                            &p,
                            cx.listener(|this, _, _, cx| {
                                this.theme = 1;
                                this.apply_theme(cx);
                                this.dirty = true;
                                this.save(cx);
                                cx.notify();
                            }),
                        ))
                        .child(seg_item(
                            "跟随系统",
                            self.theme == 2,
                            &p,
                            cx.listener(|this, _, _, cx| {
                                this.theme = 2;
                                this.apply_theme(cx);
                                this.dirty = true;
                                this.save(cx);
                                cx.notify();
                            }),
                        )),
                ),
        );
        win_card = win_card.child(divider(&p));
        win_card = win_card.child(
            h_flex()
                .gap(px(12.))
                .items_center()
                .child(row_label("关闭按钮行为", &p))
                .child(
                    h_flex()
                        .gap(px(2.))
                        .p(px(2.))
                        .rounded(px(10.))
                        .bg(theme.secondary)
                        .child(seg_item(
                            "退出程序",
                            self.close_behavior == 0,
                            &p,
                            cx.listener(|this, _, _, cx| {
                                this.close_behavior = 0;
                                this.dirty = true;
                                this.save(cx);
                                cx.notify();
                            }),
                        ))
                        .child(seg_item(
                            "最小化到托盘",
                            self.close_behavior == 1,
                            &p,
                            cx.listener(|this, _, _, cx| {
                                this.close_behavior = 1;
                                this.dirty = true;
                                this.save(cx);
                                cx.notify();
                            }),
                        )),
                ),
        );
        win_card = win_card.child(divider(&p));
        win_card = win_card.child(
            h_flex()
                .gap(px(12.))
                .items_center()
                .child(row_label("版本", &p))
                .child(
                    div()
                        .text_size(px(13.))
                        .text_color(p.muted)
                        .child("v0.1.5"),
                ),
        );

        let page = v_flex()
            .w_full()
            .max_w(px(920.))
            .gap(px(16.))
            .px(px(40.))
            .py(px(32.))
            .child(header)
            .child(account_card)
            .child(dl_card)
            .child(sec_card)
            .child(win_card);

        v_flex().flex_1().h_full().overflow_hidden().child(
            div()
                .flex_1()
                .overflow_y_scrollbar()
                .child(div().flex().justify_center().w_full().child(page)),
        )
    }
}
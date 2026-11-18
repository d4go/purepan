//! 百度账号登录页(移植 frontend/src/views/LoginView.vue)。
//! 扫码(1.5s 轮询 + 120s 倒计时)与 Cookie 两种登录方式,支持添加账号模式。

use std::sync::Arc;
use std::time::Duration;

use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{Textarea, TextareaState};
use gpui_kit::component::spinner::Spinner;
use gpui_kit::component::{ActiveTheme as _, Icon, Sizable as _, h_flex, v_flex};
use gpui_kit::{
    App, AppContext as _, BorrowAppContext as _, Context, Entity, InteractiveElement as _,
    IntoElement, ParentElement as _, Render, RenderImage, SharedString, StatefulInteractiveElement as _,
    Styled as _, Subscription, Task, Window, div, img, px,
};
use gpui_kit::prelude::FluentBuilder as _;
use purepan_backend::QRCodeStatus;

use crate::api::{self, dto::UserAuth};
use crate::app::{AppController, Screen};
use crate::bridge::ToastKind;
use crate::components::app_sidebar::Nav;
use crate::state::{AuthStore, Backend};
use crate::util;

#[derive(Clone, Copy, PartialEq, Eq)]
enum LoginTab {
    QrCode,
    Cookie,
}

pub struct LoginView {
    controller: Entity<AppController>,
    add_mode: bool,
    active_tab: LoginTab,

    qrcode: Option<purepan_backend::QRCode>,
    qr_image: Option<Arc<RenderImage>>,
    qr_loading: bool,
    qr_error: Option<String>,
    is_scanned: bool,
    is_expired: bool,
    countdown: u32,
    qr_generation: u64,
    poll_task: Option<Task<()>>,
    countdown_task: Option<Task<()>>,

    cookie_input: Entity<TextareaState>,
    cookie_loading: bool,
    cookie_error: Option<String>,
    cookie_tips_expanded: bool,

    _subscriptions: Vec<Subscription>,
}

impl LoginView {
    pub fn new(
        controller: Entity<AppController>,
        add_mode: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let cookie_input = cx.new(|cx| {
            TextareaState::new(window, cx)
                .rows(4)
                .placeholder("粘贴完整 Cookie,例如:BDUSS=xxxx; PTOKEN=yyyy; STOKEN=zzzz")
        });

        let mut view = Self {
            controller,
            add_mode,
            active_tab: LoginTab::QrCode,
            qrcode: None,
            qr_image: None,
            qr_loading: false,
            qr_error: None,
            is_scanned: false,
            is_expired: false,
            countdown: 120,
            qr_generation: 0,
            poll_task: None,
            countdown_task: None,
            cookie_input,
            cookie_loading: false,
            cookie_error: None,
            cookie_tips_expanded: false,
            _subscriptions: Vec::new(),
        };

        // 已登录用户直接进入主界面(添加账号模式除外)
        if !add_mode && cx.global::<AuthStore>().is_logged_in() {
            view.controller.update(cx, |c, cx| c.transition(Screen::Main, cx));
            return view;
        }

        view.generate_qrcode(cx);
        view
    }

    fn backend(cx: &App) -> (tokio::runtime::Handle, Arc<api::ApiClient>) {
        let b = cx.global::<Backend>();
        (b.rt.clone(), b.api.clone())
    }

    fn generate_qrcode(&mut self, cx: &mut Context<Self>) {
        self.qr_loading = true;
        self.qr_error = None;
        self.is_scanned = false;
        self.is_expired = false;
        self.qr_generation += 1;
        self.poll_task = None;
        self.countdown_task = None;
        let generation = self.qr_generation;
        let (rt, api) = Self::backend(cx);
        cx.notify();

        cx.spawn(async move |this, cx| {
            let result = api::call_api(&rt, move || async move { api.generate_qrcode().await }).await;
            let _ = this.update(cx, |this, cx| {
                if this.qr_generation != generation {
                    return;
                }
                this.qr_loading = false;
                match result {
                    Ok(qr) => {
                        this.qr_image = util::decode_data_uri_image(&qr.image_base64);
                        this.qrcode = Some(qr);
                        this.start_polling(cx);
                        this.start_countdown(cx);
                    }
                    Err(e) => this.qr_error = Some(e.message()),
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn start_polling(&mut self, cx: &mut Context<Self>) {
        let Some(qr) = &self.qrcode else { return };
        let sign = qr.sign.clone();
        let add = self.add_mode;
        let generation = self.qr_generation;
        let (rt, api) = Self::backend(cx);

        self.poll_task = Some(cx.spawn(async move |this, cx| {
            let mut first = true;
            loop {
                let (api2, sign2) = (api.clone(), sign.clone());
                let status = api::call_api(&rt, move || async move {
                    api2.qrcode_status(&sign2, add, first).await
                })
                .await;
                first = false;

                let stop = this
                    .update(cx, |this, cx| {
                        if this.qr_generation != generation {
                            return true;
                        }
                        match status {
                            Ok(QRCodeStatus::Success { user, .. }) => {
                                this.on_login_success(user, cx);
                                true
                            }
                            Ok(QRCodeStatus::Expired) => {
                                this.is_expired = true;
                                cx.notify();
                                true
                            }
                            Ok(QRCodeStatus::Failed { reason }) => {
                                this.qr_error = Some(reason);
                                cx.notify();
                                true
                            }
                            Ok(QRCodeStatus::Scanned) => {
                                this.is_scanned = true;
                                cx.notify();
                                false
                            }
                            Ok(QRCodeStatus::Waiting) => false,
                            // 轮询失败不停止,继续下一轮(与 Vue 一致)
                            Err(_) => false,
                        }
                    })
                    .unwrap_or(true);
                if stop {
                    break;
                }
                cx.background_executor().timer(Duration::from_millis(1500)).await;
                let cont = this
                    .update(cx, |this, _| this.qr_generation == generation)
                    .unwrap_or(false);
                if !cont {
                    break;
                }
            }
        }));
    }

    fn start_countdown(&mut self, cx: &mut Context<Self>) {
        self.countdown = 120;
        let generation = self.qr_generation;
        self.countdown_task = Some(cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(Duration::from_secs(1)).await;
                let cont = this
                    .update(cx, |this, cx| {
                        if this.qr_generation != generation {
                            return false;
                        }
                        this.countdown = this.countdown.saturating_sub(1);
                        if this.countdown == 0 {
                            this.is_expired = true;
                            this.poll_task = None;
                        }
                        cx.notify();
                        this.countdown > 0
                    })
                    .unwrap_or(false);
                if !cont {
                    break;
                }
            }
        }));
    }

    fn stop_qr_timers(&mut self) {
        self.qr_generation += 1;
        self.poll_task = None;
        self.countdown_task = None;
    }

    fn on_login_success(&mut self, user: UserAuth, cx: &mut Context<Self>) {
        self.stop_qr_timers();
        cx.update_global::<AuthStore, _>(|s, _| s.user = Some(user));
        self.finish_login(cx);
    }

    /// 登录/添加成功后:刷新账号列表 → 提示 → 跳转。
    fn finish_login(&mut self, cx: &mut Context<Self>) {
        let (rt, api) = Self::backend(cx);
        let add_mode = self.add_mode;
        let controller = self.controller.clone();
        cx.spawn(async move |_, cx| {
            let accounts = api::call_api(&rt, move || async move { api.list_accounts().await }).await;
            let _ = cx.update(|cx| {
                if let Ok(resp) = accounts {
                    cx.update_global::<AuthStore, _>(|s, _| {
                        s.accounts = resp.accounts;
                        s.active_uid = resp.active_uid;
                        s.accounts_loaded = true;
                    });
                }
                controller.update(cx, |c, cx| {
                    if add_mode {
                        c.toast(cx, ToastKind::Success, "账号添加成功");
                        c.navigate(Nav::Settings, cx);
                    } else {
                        c.toast(cx, ToastKind::Success, "登录成功");
                        c.transition(Screen::Main, cx);
                    }
                });
            });
        })
        .detach();
    }

    fn switch_tab(&mut self, tab: LoginTab, cx: &mut Context<Self>) {
        if self.active_tab == tab {
            return;
        }
        self.active_tab = tab;
        self.cookie_error = None;
        match tab {
            LoginTab::Cookie => self.stop_qr_timers(),
            LoginTab::QrCode => self.generate_qrcode(cx),
        }
        cx.notify();
    }

    fn login_with_cookie(&mut self, cx: &mut Context<Self>) {
        let cookies = self.cookie_input.read(cx).value().trim().to_string();
        if cookies.is_empty() || self.cookie_loading {
            return;
        }
        self.cookie_loading = true;
        self.cookie_error = None;
        cx.notify();
        let (rt, api) = Self::backend(cx);
        let add_mode = self.add_mode;
        let controller = self.controller.clone();

        cx.spawn(async move |this, cx| {
            let result =
                api::call_api(&rt, move || async move { api.cookie_login(&cookies).await }).await;
            let _ = this.update(cx, |this, cx| {
                this.cookie_loading = false;
                match result {
                    Ok((user, message)) => {
                        cx.update_global::<AuthStore, _>(|s, _| s.user = Some(user));
                        if !message.is_empty() && !message.contains("预热完成") {
                            controller.update(cx, |c, cx| {
                                c.toast(cx, ToastKind::Warning, message.clone())
                            });
                        } else {
                            controller.update(cx, |c, cx| {
                                c.toast(
                                    cx,
                                    ToastKind::Success,
                                    if add_mode { "账号添加成功" } else { "Cookie 登录成功" },
                                )
                            });
                        }
                        this.finish_login(cx);
                    }
                    Err(e) => {
                        this.cookie_error = Some(e.message());
                        cx.notify();
                    }
                }
            });
        })
        .detach();
    }

    fn render_qrcode(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        if self.qr_loading {
            return v_flex()
                .items_center()
                .gap_3()
                .child(Spinner::new().with_size(px(28.)))
                .child("正在生成二维码...")
                .into_any_element();
        }
        if let Some(err) = &self.qr_error {
            return v_flex()
                .items_center()
                .gap_3()
                .child(Icon::new(IconName::CircleX).size(px(40.)).text_color(theme.danger))
                .child(err.clone())
                .child(
                    Button::new("retry-qr")
                        .primary()
                        .small()
                        .label("重新获取")
                        .on_click(cx.listener(|this, _, _, cx| this.generate_qrcode(cx))),
                )
                .into_any_element();
        }

        let mut image_box = div()
            .relative()
            .size(px(224.))
            .p_3()
            .rounded_lg()
            .border_1()
            .border_color(theme.border)
            .bg(theme.background);
        if let Some(image) = &self.qr_image {
            image_box = image_box.child(img(image.clone()).size_full());
        }
        if self.is_expired {
            image_box = image_box.child(
                div()
                    .absolute()
                    .inset_0()
                    .flex()
                    .flex_col()
                    .items_center()
                    .justify_center()
                    .gap_2()
                    .rounded_lg()
                    .bg(gpui_kit::black().opacity(0.75))
                    .text_color(gpui_kit::white())
                    .child(Icon::new(IconName::RefreshCw).size(px(36.)))
                    .child("二维码已过期")
                    .child(
                        Button::new("refresh-qr")
                            .primary()
                            .small()
                            .label("刷新二维码")
                            .on_click(cx.listener(|this, _, _, cx| this.generate_qrcode(cx))),
                    ),
            );
        } else if self.is_scanned {
            image_box = image_box.child(
                div()
                    .absolute()
                    .inset_0()
                    .flex()
                    .flex_col()
                    .items_center()
                    .justify_center()
                    .gap_2()
                    .rounded_lg()
                    .bg(theme.success.opacity(0.15))
                    .text_color(theme.success)
                    .child(Icon::new(IconName::CircleCheck).size(px(40.)))
                    .child("请在手机上确认登录"),
            );
        }

        let tip: SharedString = if self.is_scanned {
            "请在手机百度 APP 中点击\"确认登录\"".into()
        } else {
            "请使用百度 APP 扫描二维码登录".into()
        };

        v_flex()
            .items_center()
            .gap_3()
            .child(image_box)
            .child(
                h_flex()
                    .gap_1p5()
                    .items_center()
                    .px_3()
                    .py_2()
                    .rounded_md()
                    .bg(if self.is_scanned { theme.success.opacity(0.12) } else { theme.primary.opacity(0.1) })
                    .text_color(if self.is_scanned { theme.success } else { theme.primary })
                    .child(Icon::new(if self.is_scanned { IconName::Smartphone } else { IconName::QrCode }).size(px(16.)))
                    .child(tip),
            )
            .child(
                div()
                    .text_color(theme.muted_foreground)
                    .child(format!("{}秒后自动刷新", self.countdown)),
            )
            .into_any_element()
    }

    fn render_cookie(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let mut tips = v_flex()
            .rounded_md()
            .border_1()
            .border_color(theme.border)
            .bg(theme.info.opacity(0.08))
            .child(
                h_flex()
                    .id("cookie-tips-header")
                    .gap_1p5()
                    .items_center()
                    .px_3()
                    .py_2()
                    .cursor_pointer()
                    .text_color(theme.primary)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.cookie_tips_expanded = !this.cookie_tips_expanded;
                        cx.notify();
                    }))
                    .child(Icon::new(IconName::Info).size(px(14.)))
                    .child(div().flex_1().child("如何获取 Cookie?"))
                    .child(Icon::new(if self.cookie_tips_expanded { IconName::ChevronUp } else { IconName::ChevronDown }).size(px(14.))),
            );
        if self.cookie_tips_expanded {
            tips = tips.child(
                v_flex()
                    .px_3()
                    .pb_2()
                    .gap_1()
                    .text_color(theme.muted_foreground)
                    .child("1. 浏览器打开 pan.baidu.com 并登录")
                    .child("2. 按 F12 → Network 标签页")
                    .child("3. 刷新页面,点击任意请求")
                    .child("4. 在 Headers → Request Headers 中找到 cookie")
                    .child("5. 复制完整值,粘贴到下方输入框"),
            );
        }

        let mut section = v_flex().gap_3().child(tips).child(Textarea::new(&self.cookie_input));
        if let Some(err) = &self.cookie_error {
            section = section.child(
                h_flex()
                    .gap_1p5()
                    .items_center()
                    .px_3()
                    .py_2()
                    .rounded_md()
                    .bg(theme.danger.opacity(0.1))
                    .text_color(theme.danger)
                    .child(Icon::new(IconName::CircleX).size(px(13.)))
                    .child(err.clone()),
            );
        }
        let loading = self.cookie_loading;
        section = section.child(
            Button::new("cookie-login")
                .primary()
                .loading(loading)
                .label(if loading { "登录中..." } else { "使用 Cookie 登录" })
                .on_click(cx.listener(|this, _, _, cx| this.login_with_cookie(cx))),
        );
        section.into_any_element()
    }
}

impl Render for LoginView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let mut content = v_flex().flex_1().px_6().py_4().gap_4();

        if self.add_mode {
            let count = cx.global::<AuthStore>().accounts.len();
            content = content.child(
                h_flex()
                    .items_center()
                    .justify_between()
                    .px_3()
                    .py_2()
                    .rounded_md()
                    .bg(theme.info.opacity(0.1))
                    .child(format!("添加新账号(已有 {count} 个)"))
                    .child(
                        Button::new("back-settings")
                            .ghost()
                            .small()
                            .label("返回")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.controller.update(cx, |c, cx| c.navigate(Nav::Settings, cx));
                            })),
                    ),
            );
        }

        // 登录方式切换
        let tab = self.active_tab;
        content = content.child(
            h_flex()
                .gap_1()
                .p_1()
                .rounded_lg()
                .bg(theme.secondary)
                .child(
                    Button::new("tab-qrcode")
                        .when(tab == LoginTab::QrCode, |b| b.primary())
                        .when(tab != LoginTab::QrCode, |b| b.ghost())
                        .flex_1()
                        .icon(IconName::QrCode)
                        .label("扫码登录")
                        .on_click(cx.listener(|this, _, _, cx| this.switch_tab(LoginTab::QrCode, cx))),
                )
                .child(
                    Button::new("tab-cookie")
                        .when(tab == LoginTab::Cookie, |b| b.primary())
                        .when(tab != LoginTab::Cookie, |b| b.ghost())
                        .flex_1()
                        .icon(IconName::KeyRound)
                        .label("Cookie 登录")
                        .on_click(cx.listener(|this, _, _, cx| this.switch_tab(LoginTab::Cookie, cx))),
                ),
        );

        content = content.child(
            div()
                .flex_1()
                .flex()
                .flex_col()
                .justify_center()
                .child(match self.active_tab {
                    LoginTab::QrCode => self.render_qrcode(cx).into_any_element(),
                    LoginTab::Cookie => self.render_cookie(cx).into_any_element(),
                }),
        );

        content = content.child(
            div()
                .text_center()
                .text_color(theme.muted_foreground)
                .child("v0.1.5"),
        );

        content
    }
}
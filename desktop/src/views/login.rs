//! 百度账号登录页(移植 frontend/src/views/LoginView.vue)。
//! 扫码(1.5s 轮询 + 120s 倒计时)与 Cookie 两种登录方式,支持添加账号模式。

use std::sync::Arc;
use std::time::Duration;

use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{Textarea, TextareaState};
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::spinner::Spinner;
use gpui_kit::component::{ActiveTheme as _, Disableable as _, Icon, Sizable as _, h_flex, v_flex};
use gpui_kit::{
    AnyElement, App, AppContext as _, BorrowAppContext as _, Context, Entity,
    FontWeight, InteractiveElement as _, IntoElement, ParentElement as _, Render, RenderImage,
    Styled as _, Subscription, Task, Window, div, img, linear_color_stop, linear_gradient, px,
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
    qr_blur: Option<Arc<RenderImage>>,
    qr_loading: bool,
    qr_error: Option<String>,
    is_scanned: bool,
    login_success: bool,
    canceled: bool,
    cancel_seconds: u32,
    is_expired: bool,
    countdown: u32,
    qr_generation: u64,
    poll_task: Option<Task<()>>,
    countdown_task: Option<Task<()>>,
    cancel_task: Option<Task<()>>,

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
                .placeholder("粘贴完整的 Cookie 请求头值")
        });

        let mut view = Self {
            controller,
            add_mode,
            active_tab: LoginTab::QrCode,
            qrcode: None,
            qr_image: None,
            qr_blur: None,
            qr_loading: false,
            qr_error: None,
            is_scanned: false,
            login_success: false,
            canceled: false,
            cancel_seconds: 6,
            is_expired: false,
            countdown: 120,
            qr_generation: 0,
            poll_task: None,
            countdown_task: None,
            cancel_task: None,
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
        self.login_success = false;
        self.canceled = false;
        self.cancel_seconds = 6;
        self.is_expired = false;
        self.qr_generation += 1;
        self.poll_task = None;
        self.countdown_task = None;
        self.cancel_task = None;
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
                    Ok(qr) => match util::decode_data_uri_image_blurred(&qr.image_base64, 18.0) {
                        Some((image, blurred)) => {
                            this.qr_image = Some(image);
                            this.qr_blur = Some(blurred);
                            this.qrcode = Some(qr);
                            this.start_polling(cx);
                            this.start_countdown(cx);
                        }
                        None => this.qr_error = Some("二维码图片无法显示，请重新获取".into()),
                    },
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
                                // 先展示成功蒙层,再切换主界面,避免扫码确认后画面"闪一下就走"
                                this.is_scanned = true;
                                this.login_success = true;
                                this.countdown_task = None;
                                cx.notify();
                                let entity = cx.entity().downgrade();
                                cx.spawn(async move |_, cx| {
                                    cx.background_executor()
                                        .timer(Duration::from_millis(900))
                                        .await;
                                    let _ = entity.update(cx, move |this, cx| {
                                        this.on_login_success(user, cx)
                                    });
                                })
                                .detach();
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
                            Ok(QRCodeStatus::Canceled) => {
                                this.on_canceled(cx);
                                true
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
        self.cancel_task = None;
    }

    /// 手机端取消登录:优雅提示 + 6 秒后自动刷新二维码。
    fn on_canceled(&mut self, cx: &mut Context<Self>) {
        self.stop_qr_timers();
        self.canceled = true;
        self.cancel_seconds = 6;
        cx.notify();
        self.cancel_task = Some(cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_secs(1))
                    .await;
                let keep_going = this
                    .update(cx, |this, cx| {
                        if !this.canceled {
                            return false;
                        }
                        this.cancel_seconds = this.cancel_seconds.saturating_sub(1);
                        if this.cancel_seconds == 0 {
                            this.generate_qrcode(cx);
                            return false;
                        }
                        cx.notify();
                        true
                    })
                    .unwrap_or(false);
                if !keep_going {
                    break;
                }
            }
        }));
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
        if self.active_tab == tab || self.login_success || self.cookie_loading {
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
                        this.cookie_loading = false;
                        this.cookie_error = Some(e.message());
                        cx.notify();
                    }
                }
            });
        })
        .detach();
    }

    fn render_qr_overlay(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let theme = cx.theme().clone();
        let (icon, accent, title, detail, action) = if self.login_success {
            (
                IconName::Check,
                theme.success,
                if self.add_mode {
                    "账号添加成功"
                } else {
                    "登录成功"
                },
                Some(
                    if self.add_mode {
                        "正在返回账号设置…"
                    } else {
                        "正在进入主界面…"
                    }
                    .into(),
                ),
                None,
            )
        } else if self.canceled {
            (
                IconName::RefreshCw,
                theme.muted_foreground,
                "已取消登录",
                Some(format!("{} 秒后自动刷新二维码", self.cancel_seconds)),
                Some(
                    Button::new("qr-scan-again")
                        .ghost()
                        .small()
                        .label("立即刷新")
                        .on_click(cx.listener(|this, _, _, cx| this.generate_qrcode(cx))),
                ),
            )
        } else if self.is_expired {
            (
                IconName::RefreshCw,
                theme.muted_foreground,
                "二维码已过期",
                None,
                Some(
                    Button::new("refresh-qr")
                        .primary()
                        .small()
                        .label("刷新二维码")
                        .on_click(cx.listener(|this, _, _, cx| this.generate_qrcode(cx))),
                ),
            )
        } else if self.is_scanned {
            (
                IconName::Smartphone,
                theme.primary,
                "已扫码",
                Some("请在手机百度 App 上确认登录".into()),
                None,
            )
        } else {
            return None;
        };

        Some(
            v_flex()
                .absolute()
                .inset_0()
                .items_center()
                .justify_center()
                .gap(px(10.))
                .px(px(16.))
                .text_center()
                .bg(linear_gradient(
                    155.,
                    linear_color_stop(theme.background.opacity(0.9), 0.),
                    linear_color_stop(theme.background.opacity(0.98), 1.),
                ))
                .child(
                    div()
                        .size(px(62.))
                        .rounded_full()
                        .border_1()
                        .border_color(accent.opacity(0.2))
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(
                            div()
                                .size(px(46.))
                                .rounded_full()
                                .bg(accent.opacity(0.14))
                                .flex()
                                .items_center()
                                .justify_center()
                                .child(Icon::new(icon).size(px(24.)).text_color(accent)),
                        ),
                )
                .child(
                    div()
                        .text_size(px(17.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(theme.foreground)
                        .child(title),
                )
                .when_some(detail, |overlay, detail| {
                    overlay.child(
                        div()
                            .text_size(px(12.))
                            .text_color(theme.muted_foreground)
                            .child(detail),
                    )
                })
                .when_some(action, |overlay, action| overlay.child(action))
                .into_any_element(),
        )
    }

    fn render_qrcode(&self, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        let frame = div()
            .relative()
            .size(px(224.))
            .flex_shrink_0()
            .rounded(px(20.))
            .overflow_hidden()
            .border_1()
            .border_color(theme.border);

        let mut waiting_for_scan = false;
        let frame = if self.qr_loading {
            frame.bg(theme.background).child(
                v_flex()
                    .size_full()
                    .items_center()
                    .justify_center()
                    .gap(px(10.))
                    .text_size(px(13.))
                    .text_color(theme.muted_foreground)
                    .child(Spinner::new().with_size(px(24.)))
                    .child("正在生成二维码…"),
            )
        } else if let Some(err) = &self.qr_error {
            frame.bg(theme.background).child(
                v_flex()
                    .size_full()
                    .px(px(14.))
                    .items_center()
                    .justify_center()
                    .gap(px(10.))
                    .text_center()
                    .text_size(px(12.))
                    .text_color(theme.muted_foreground)
                    .child(
                        Icon::new(IconName::CircleX)
                            .size(px(26.))
                            .text_color(theme.danger),
                    )
                    .child(err.clone())
                    .child(
                        Button::new("retry-qr")
                            .primary()
                            .small()
                            .label("重新获取")
                            .on_click(cx.listener(|this, _, _, cx| this.generate_qrcode(cx))),
                    ),
            )
        } else {
            let overlay = self.render_qr_overlay(cx);
            waiting_for_scan = overlay.is_none();
            let image = if waiting_for_scan {
                &self.qr_image
            } else {
                &self.qr_blur
            };
            frame
                .bg(gpui_kit::white())
                .p(px(12.))
                .when_some(image.clone(), |frame, image| {
                    frame.child(img(image).size_full())
                })
                .when_some(overlay, |frame, overlay| frame.child(overlay))
        };

        v_flex()
            .items_center()
            .gap(px(14.))
            .child(frame)
            .child(
                v_flex()
                    .h(px(44.))
                    .items_center()
                    .gap(px(4.))
                    .when(waiting_for_scan, |hint| {
                        hint.child(div().text_size(px(13.)).child("请使用百度 App 扫码登录"))
                            .child(
                                div()
                                    .text_size(px(11.))
                                    .text_color(theme.muted_foreground)
                                    .child(format!("二维码将在 {} 秒后过期", self.countdown)),
                            )
                    }),
            )
            .into_any_element()
    }

    fn render_cookie(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let expanded = self.cookie_tips_expanded;
        let tips = v_flex()
            .gap(px(6.))
            .child(
                h_flex().child(
                    Button::new("cookie-tips-header")
                        .ghost()
                        .small()
                        .icon(IconName::Info)
                        .label("如何获取 Cookie？")
                        .child(
                            Icon::new(if expanded {
                                IconName::ChevronUp
                            } else {
                                IconName::ChevronDown
                            })
                            .size(px(12.)),
                        )
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.cookie_tips_expanded = !this.cookie_tips_expanded;
                            cx.notify();
                        })),
                ),
            )
            .when(expanded, |tips| {
                tips.child(
                    v_flex()
                        .gap(px(6.))
                        .px(px(12.))
                        .py(px(10.))
                        .rounded(px(10.))
                        .border_1()
                        .border_color(theme.border)
                        .bg(theme.secondary.opacity(0.55))
                        .text_size(px(12.))
                        .text_color(theme.muted_foreground)
                        .child("1. 在浏览器打开 pan.baidu.com 并登录。")
                        .child("2. 打开开发者工具的「网络」面板，刷新页面并选中一条网盘请求。")
                        .child("3. 在「请求标头」中找到 Cookie，复制完整值并粘贴到输入框。"),
                )
            });

        v_flex()
            .gap(px(12.))
            .flex_shrink_0()
            .child(
                v_flex()
                    .gap(px(8.))
                    .child(
                        div()
                            .text_size(px(13.))
                            .font_weight(FontWeight::MEDIUM)
                            .child("百度网盘 Cookie"),
                    )
                    .child(
                        Textarea::new(&self.cookie_input)
                            .large()
                            .disabled(self.cookie_loading),
                    )
                    .when_some(self.cookie_error.clone(), |field, err| {
                        field.child(
                            h_flex()
                                .items_start()
                                .gap(px(6.))
                                .px(px(10.))
                                .py(px(8.))
                                .rounded(px(8.))
                                .bg(theme.danger.opacity(0.09))
                                .text_color(theme.danger)
                                .text_size(px(12.))
                                .child(Icon::new(IconName::CircleX).size(px(14.)).flex_shrink_0())
                                .child(div().min_w_0().flex_1().child(err)),
                        )
                    }),
            )
            .child(
                Button::new("cookie-login")
                    .primary()
                    .w_full()
                    .loading(self.cookie_loading)
                    .label(if self.cookie_loading {
                        "正在登录…"
                    } else {
                        "使用 Cookie 登录"
                    })
                    .on_click(cx.listener(|this, _, _, cx| this.login_with_cookie(cx))),
            )
            .child(tips)
    }
}

impl Render for LoginView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let finishing = self.login_success || self.cookie_loading;
        let mut content = v_flex().flex_1().min_h_0().px_6().py_4().gap_4();

        if self.add_mode {
            let count = cx.global::<AuthStore>().accounts.len();
            content = content.child(
                h_flex()
                    .flex_shrink_0()
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
                            .disabled(finishing)
                            .label("返回")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.controller
                                    .update(cx, |c, cx| c.navigate(Nav::Settings, cx));
                            })),
                    ),
            );
        }

        // 登录方式切换
        let tab = self.active_tab;
        content = content.child(
            h_flex()
                .flex_shrink_0()
                .gap_1()
                .p_1()
                .rounded_lg()
                .bg(theme.secondary)
                .child(
                    Button::new("tab-qrcode")
                        .when(tab == LoginTab::QrCode, |b| b.primary())
                        .when(tab != LoginTab::QrCode, |b| b.ghost())
                        .flex_1()
                        .disabled(finishing)
                        .icon(IconName::QrCode)
                        .label("扫码登录")
                        .on_click(
                            cx.listener(|this, _, _, cx| this.switch_tab(LoginTab::QrCode, cx)),
                        ),
                )
                .child(
                    Button::new("tab-cookie")
                        .when(tab == LoginTab::Cookie, |b| b.primary())
                        .when(tab != LoginTab::Cookie, |b| b.ghost())
                        .flex_1()
                        .disabled(finishing)
                        .icon(IconName::KeyRound)
                        .label("Cookie 登录")
                        .on_click(
                            cx.listener(|this, _, _, cx| this.switch_tab(LoginTab::Cookie, cx)),
                        ),
                ),
        );

        let login_content = match tab {
            LoginTab::QrCode => v_flex()
                .flex_1()
                .min_h_0()
                .justify_center()
                .child(self.render_qrcode(cx))
                .into_any_element(),
            LoginTab::Cookie => v_flex()
                .id("cookie-login-scroll")
                .flex_1()
                .min_h_0()
                .overflow_y_scrollbar()
                .child(self.render_cookie(cx))
                .into_any_element(),
        };

        content.child(login_content).child(
            v_flex()
                .flex_shrink_0()
                .items_center()
                .gap(px(4.))
                .text_size(px(11.))
                .text_color(theme.muted_foreground)
                .child(img(util::app_logo()).size(px(28.)).rounded(px(7.)))
                .child(concat!("v", env!("CARGO_PKG_VERSION"))),
        )
    }
}

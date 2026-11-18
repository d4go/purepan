//! Web 访问认证登录页(移植 frontend/src/views/WebLoginView.vue)。
//! 密码 → TOTP 两步,恢复码对话框,速率限制倒计时。

use std::sync::Arc;
use std::time::Duration;

use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::dialog::Dialog;
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::progress::Progress;
use gpui_kit::component::{
    ActiveTheme as _, Disableable as _, Icon, Sizable as _, WindowExt as _, h_flex, v_flex,
};
use gpui_kit::{
    AppContext as _, BorrowAppContext as _, Context, Entity, IntoElement, ParentElement as _,
    Render, Styled as _, Subscription, Task, Window, div, px,
};

use crate::api::{self, dto::WebLoginRequest};
use crate::app::AppController;
use crate::state::{Backend, LoginStep, WebAuthState};

pub struct WebLoginView {
    controller: Entity<AppController>,
    password: Entity<InputState>,
    totp: Entity<InputState>,
    recovery: Entity<InputState>,
    is_loading: bool,
    lockout_task: Option<Task<()>>,
    _subscriptions: Vec<Subscription>,
}

impl WebLoginView {
    pub fn new(
        controller: Entity<AppController>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let password = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("请输入访问密码")
                .masked(true)
        });
        let totp = cx.new(|cx| InputState::new(window, cx).placeholder("请输入 6 位验证码"));
        let recovery = cx.new(|cx| InputState::new(window, cx).placeholder("XXXX-XXXX"));

        let mut subs = Vec::new();
        // Enter 提交
        subs.push(cx.subscribe_in(&password, window, |this, _, ev, _, cx| {
            if matches!(ev, InputEvent::PressEnter { .. }) {
                this.submit_password(cx);
            }
        }));
        subs.push(cx.subscribe_in(&totp, window, |this, _, ev, _, cx| {
            if matches!(ev, InputEvent::PressEnter { .. }) {
                this.submit_totp(cx);
            }
        }));

        Self {
            controller,
            password,
            totp,
            recovery,
            is_loading: false,
            lockout_task: None,
            _subscriptions: subs,
        }
    }

    fn backend(cx: &Context<Self>) -> (tokio::runtime::Handle, Arc<api::ApiClient>) {
        let b = cx.global::<Backend>();
        (b.rt.clone(), b.api.clone())
    }

    fn start_lockout_countdown(&mut self, cx: &mut Context<Self>) {
        self.lockout_task = Some(cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(Duration::from_secs(1)).await;
                let cont = this
                    .update(cx, |_, cx| {
                        let mut more = false;
                        cx.update_global::<WebAuthState, _>(|s, _| {
                            if let Some(remain) = s.lockout_remaining {
                                let next = remain - 1;
                                s.lockout_remaining = if next > 0 { Some(next) } else { None };
                                more = next > 0;
                            }
                        });
                        cx.notify();
                        more
                    })
                    .unwrap_or(false);
                if !cont {
                    break;
                }
            }
        }));
    }

    fn submit_password(&mut self, cx: &mut Context<Self>) {
        let password = self.password.read(cx).value().to_string();
        if password.is_empty() || self.is_loading || self.is_locked(cx) {
            return;
        }
        self.login(
            WebLoginRequest {
                password: Some(password),
                totp_code: None,
                recovery_code: None,
                pending_token: None,
            },
            cx,
        );
    }

    fn submit_totp(&mut self, cx: &mut Context<Self>) {
        let code = self.totp.read(cx).value().to_string();
        if code.is_empty() || self.is_loading || self.is_locked(cx) {
            return;
        }
        let pending = cx.global::<WebAuthState>().pending_login_token.clone();
        self.login(
            WebLoginRequest {
                password: None,
                totp_code: Some(code),
                recovery_code: None,
                pending_token: pending,
            },
            cx,
        );
    }

    fn submit_recovery(&mut self, cx: &mut Context<Self>) {
        let code = self.recovery.read(cx).value().to_string();
        if code.is_empty() || self.is_loading {
            return;
        }
        let pending = cx.global::<WebAuthState>().pending_login_token.clone();
        self.login(
            WebLoginRequest {
                password: None,
                totp_code: None,
                recovery_code: Some(code),
                pending_token: pending,
            },
            cx,
        );
    }

    fn is_locked(&self, cx: &Context<Self>) -> bool {
        cx.global::<WebAuthState>()
            .lockout_remaining
            .map(|r| r > 0)
            .unwrap_or(false)
    }

    fn login(&mut self, req: WebLoginRequest, cx: &mut Context<Self>) {
        self.is_loading = true;
        cx.update_global::<WebAuthState, _>(|s, _| {
            s.error = None;
            s.lockout_remaining = None;
        });
        cx.notify();
        let (rt, api) = Self::backend(cx);
        let controller = self.controller.clone();

        cx.spawn(async move |this, cx| {
            let result = api::call_api(&rt, move || async move { api.web_login(req).await }).await;
            let _ = this.update(cx, |this, cx| {
                this.is_loading = false;
                match result {
                    Ok(resp) => match resp.status.as_str() {
                        "success" => {
                            let api = cx.global::<Backend>().api.clone();
                            let rt = cx.global::<Backend>().rt.clone();
                            let resp2 = resp.clone();
                            cx.spawn(async move |_, cx| {
                                let _ = api::call_api(&rt, move || async move {
                                    api.save_login_tokens(&resp2).await;
                                    Ok::<_, api::ApiError>(())
                                })
                                .await;
                                let _ = cx.update(|cx| {
                                    cx.update_global::<WebAuthState, _>(|s, _| {
                                        s.is_authenticated = true;
                                        s.login_step = LoginStep::Complete;
                                        s.pending_login_token = None;
                                    });
                                    controller.update(cx, |c, cx| c.proceed_after_web_auth(cx));
                                });
                            })
                            .detach();
                        }
                        "need_totp" => {
                            cx.update_global::<WebAuthState, _>(|s, _| {
                                s.pending_login_token = resp.pending_token.clone();
                                s.login_step = LoginStep::Totp;
                            });
                        }
                        _ => {
                            let lockout = resp.lockout_remaining;
                            cx.update_global::<WebAuthState, _>(|s, _| {
                                s.error = resp.error.clone().or_else(|| Some("登录失败".into()));
                                s.lockout_remaining = lockout;
                            });
                            if lockout.map(|r| r > 0).unwrap_or(false) {
                                this.start_lockout_countdown(cx);
                            }
                        }
                    },
                    Err(e) => {
                        cx.update_global::<WebAuthState, _>(|s, _| s.error = Some(e.message()));
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn open_recovery_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let view = cx.entity().clone();
        let recovery = self.recovery.clone();
        window.open_dialog(cx, move |dialog: Dialog, _, _| {
            let view2 = view.clone();
            let view3 = view.clone();
            dialog
                .title("使用恢复码登录")
                .width(px(360.))
                .child(
                    v_flex()
                        .gap_3()
                        .child("请输入您保存的恢复码,每个恢复码只能使用一次")
                        .child(Input::new(&recovery)),
                )
                .footer(
                    h_flex()
                        .gap_2()
                        .justify_end()
                        .child(
                            Button::new("recovery-cancel")
                                .label("取消")
                                .on_click(move |_, window, cx| window.close_dialog(cx)),
                        )
                        .child(
                            Button::new("recovery-ok")
                                .primary()
                                .label("验证")
                                .on_click(move |_, _, cx| {
                                    view2.update(cx, |this, cx| this.submit_recovery(cx));
                                }),
                        ),
                )
                .on_ok(move |_, _, cx| {
                    view3.update(cx, |this, cx| this.submit_recovery(cx));
                    true
                })
        });
    }
}

impl Render for WebLoginView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let state = cx.global::<WebAuthState>();
        let step = state.login_step;
        let requires_password = state.requires_password();
        let requires_totp = state.requires_totp();
        let error = state.error.clone();
        let lockout = state.lockout_remaining;
        let locked = lockout.map(|r| r > 0).unwrap_or(false);
        let loading = self.is_loading;

        let show_password = step == LoginStep::Password && requires_password;
        let show_totp = step == LoginStep::Totp || (step == LoginStep::Password && !requires_password && requires_totp);

        let subtitle = if show_password {
            "请输入访问密码"
        } else if requires_password && requires_totp {
            "请输入两步验证码"
        } else {
            "请输入验证码"
        };

        let mut form = v_flex().gap_3().w_full();

        if show_password {
            form = form
                .child(Input::new(&self.password).large())
                .child(
                    Button::new("web-login-submit")
                        .primary()
                        .large()
                        .loading(loading)
                        .disabled(locked)
                        .label(if requires_totp { "下一步" } else { "登录" })
                        .on_click(cx.listener(|this, _, _, cx| this.submit_password(cx))),
                );
        }

        if show_totp {
            if step == LoginStep::Totp && requires_password {
                form = form.child(
                    Button::new("back-password")
                        .link()
                        .small()
                        .icon(IconName::ArrowLeft)
                        .label("返回密码输入")
                        .on_click(cx.listener(|_, _, _, cx| {
                            cx.update_global::<WebAuthState, _>(|s, _| {
                                s.login_step = LoginStep::Password;
                                s.pending_login_token = None;
                                s.error = None;
                            });
                            cx.notify();
                        })),
                );
            }
            form = form
                .child(Input::new(&self.totp).large())
                .child(
                    Button::new("web-totp-submit")
                        .primary()
                        .large()
                        .loading(loading)
                        .disabled(locked)
                        .label("验证")
                        .on_click(cx.listener(|this, _, _, cx| this.submit_totp(cx))),
                )
                .child(
                    Button::new("use-recovery")
                        .link()
                        .small()
                        .icon(IconName::Ticket)
                        .label("使用恢复码登录")
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.open_recovery_dialog(window, cx)
                        })),
                );
        }

        if let Some(err) = error {
            form = form.child(
                h_flex()
                    .gap_1p5()
                    .items_center()
                    .px_3()
                    .py_2()
                    .rounded_md()
                    .bg(theme.danger.opacity(0.1))
                    .text_color(theme.danger)
                    .child(Icon::new(IconName::CircleAlert).size(px(15.)))
                    .child(err),
            );
        }

        if let Some(remain) = lockout.filter(|r| *r > 0) {
            let progress = (remain as f32 / 900.0 * 100.0).clamp(0.0, 100.0);
            form = form.child(
                v_flex()
                    .gap_2()
                    .px_3()
                    .py_2()
                    .rounded_md()
                    .bg(theme.warning.opacity(0.1))
                    .text_color(theme.warning)
                    .child(format!("请求过于频繁,请在 {remain} 秒后重试"))
                    .child(Progress::new("lockout").value(progress)),
            );
        }

        v_flex()
            .flex_1()
            .items_center()
            .justify_center()
            .px_8()
            .child(
                v_flex()
                    .items_center()
                    .gap_2()
                    .w_full()
                    .child(Icon::new(IconName::Lock).size(px(28.)).text_color(theme.primary))
                    .child(div().text_size(px(20.)).child("Web 访问认证"))
                    .child(div().text_color(theme.muted_foreground).child(subtitle))
                    .child(div().h_4())
                    .child(form),
            )
    }
}
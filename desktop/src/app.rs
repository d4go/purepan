//! 应用控制器(常驻,跨窗口重建存活)+ 每窗口 UI 壳 AppShell。
//!
//! 控制器持有:屏幕状态机、导航、事件泵(WS→Global/视图)、健康轮询、
//! 令牌主动刷新、启动引导。窗口在 登录(400×600)↔ 主界面(1280×860)
//! 切换时重建(Global 状态存活),对齐 Vue 的 windowManager 行为。

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use gpui_kit::component::spinner::Spinner;
use gpui_kit::component::status_bar::StatusBar;
use gpui_kit::component::{ActiveTheme as _, Sizable as _, TitleBar, h_flex};
use gpui_kit::{
    AnyView, AnyWindowHandle, App, AppContext as _, BorrowAppContext as _, Bounds, Context, Entity,
    IntoElement, ParentElement as _, Render, Styled as _, Subscription, Task, Window, WindowBounds,
    WindowOptions, div, px, size,
};
use purepan_backend::server::events::TaskEvent;
use purepan_backend::server::websocket::WsServerMessage;

use crate::api::{self, ApiClient};
use crate::bridge::{ConnectionState, EventHub, ToastKind, UiEvent};
use crate::components::{app_sidebar, app_sidebar::Nav, app_title_bar, toast};
use crate::state::{AuthStore, Backend, BackendHealth, WebAuthState};
use crate::views::{login::LoginView, placeholder::Placeholder, web_login::WebLoginView};
use crate::{bridge, shell};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Boot,
    WebLogin,
    Login,
    Main,
}

pub struct AppController {
    pub screen: Screen,
    pub nav: Nav,
    pub login_add_mode: bool,
    pub ws_state: ConnectionState,
    window: Option<AnyWindowHandle>,
    pub hub: Entity<EventHub>,
    rt: tokio::runtime::Handle,
    api: Arc<ApiClient>,
    http: reqwest::Client,
    health_url: String,
    ws: crate::ws::WsHandle,
    booted: bool,
    last_active_uid: Option<u64>,
    _pump: Task<()>,
    _health: Task<()>,
    _refresh: Task<()>,
}

impl AppController {
    pub fn new(cx: &mut Context<Self>, ui_rx: async_channel::Receiver<UiEvent>) -> Self {
        let b = cx.global::<Backend>();
        let rt = b.rt.clone();
        let api = b.api.clone();
        let http = b.http.clone();
        let ws = b.ws.clone();
        let health_url = format!("{}/health", b.base_url);
        let hub = cx.new(|_| EventHub);

        // 事件泵:WS/ApiClient → Global 状态 + 广播给视图
        let pump = cx.spawn(async move |this, cx| {
            while let Ok(ev) = ui_rx.recv().await {
                if this.update(cx, |this, cx| this.handle_event(ev, cx)).is_err() {
                    break;
                }
            }
        });

        // 健康轮询(2s),断连/恢复各提示一次(启动完成后才提示)
        let health = cx.spawn({
            let (rt, http, url) = (rt.clone(), http.clone(), health_url.clone());
            async move |this, cx| {
                loop {
                    let (client, u) = (http.clone(), url.clone());
                    let ok = bridge::call(&rt, move || async move {
                        client
                            .get(&u)
                            .timeout(Duration::from_secs(2))
                            .send()
                            .await
                            .map(|r| r.status().is_success())
                            .unwrap_or(false)
                    })
                    .await
                    .unwrap_or(false);
                    let _ = this.update(cx, |this, cx| {
                        let was = cx.global::<BackendHealth>().connected;
                        cx.update_global::<BackendHealth, _>(|h, _| h.connected = ok);
                        if this.booted {
                            if was && !ok {
                                this.toast(cx, ToastKind::Error, "无法连接后端服务,正在尝试重连…");
                            } else if !was && ok {
                                this.toast(cx, ToastKind::Success, "后端服务已恢复连接");
                            }
                        }
                        cx.notify();
                    });
                    cx.background_executor().timer(Duration::from_secs(2)).await;
                }
            }
        });

        // 令牌主动刷新(过期前 5 分钟,30s 检查一次)
        let refresh = cx.spawn({
            let (rt, api) = (rt.clone(), api.clone());
            async move |_this, cx| {
                loop {
                    cx.background_executor().timer(Duration::from_secs(30)).await;
                    let api2 = api.clone();
                    let _ = bridge::call(&rt, move || async move {
                        api2.refresh_if_expiring().await;
                    })
                    .await;
                }
            }
        });

        Self {
            screen: Screen::Boot,
            nav: Nav::Files,
            login_add_mode: false,
            ws_state: ConnectionState::Disconnected,
            window: None,
            hub,
            rt,
            api,
            http,
            health_url,
            ws,
            booted: false,
            last_active_uid: None,
            _pump: pump,
            _health: health,
            _refresh: refresh,
        }
    }

    /// 启动引导任务(窗口已由 open_initial 在 App 上下文打开,避免实体更新内开窗)。
    fn spawn_boot(&mut self, cx: &mut Context<Self>) {
        let (rt, api, http, url) = (
            self.rt.clone(),
            self.api.clone(),
            self.http.clone(),
            self.health_url.clone(),
        );
        cx.spawn(async move |this, cx| {
            // 1. 等待后端就绪(最多 30s,100ms→1s 退避)
            let start = Instant::now();
            let mut delay = Duration::from_millis(100);
            loop {
                let (client, u) = (http.clone(), url.clone());
                let ok = bridge::call(&rt, move || async move {
                    client
                        .get(&u)
                        .timeout(Duration::from_secs(2))
                        .send()
                        .await
                        .map(|r| r.status().is_success())
                        .unwrap_or(false)
                })
                .await
                .unwrap_or(false);
                if ok || start.elapsed() > Duration::from_secs(30) {
                    break;
                }
                cx.background_executor().timer(delay).await;
                delay = (delay * 2).min(Duration::from_secs(1));
            }

            // 2. 本地令牌 → 初始 Web 认证态
            let tokens = bridge::call(&rt, {
                let api = api.clone();
                move || async move { api.tokens_snapshot().await }
            })
            .await
            .unwrap_or_default();
            let now = chrono::Utc::now().timestamp();
            let token_valid = tokens
                .access_token
                .is_some()
                && tokens.access_expires_at.map(|e| e > now).unwrap_or(false);

            // 3. Web 认证状态 + 百度账号(并行语义,顺序调用即可)
            let status = api::call_api(&rt, {
                let api = api.clone();
                move || async move { api.web_status().await }
            })
            .await
            .ok();
            let accounts = api::call_api(&rt, {
                let api = api.clone();
                move || async move { api.list_accounts().await }
            })
            .await;
            let user_fallback = if accounts.is_err() {
                api::call_api(&rt, {
                    let api = api.clone();
                    move || async move { api.current_user().await }
                })
                .await
                .ok()
            } else {
                None
            };

            let _ = this.update(cx, |this, cx| {
                cx.update_global::<WebAuthState, _>(|s, _| {
                    if let Some(status) = status {
                        s.auth_config = Some(status.clone());
                        s.is_authenticated = !status.enabled || token_valid;
                    } else if token_valid {
                        s.is_authenticated = true;
                    }
                });
                match accounts {
                    Ok(resp) => cx.update_global::<AuthStore, _>(|s, _| {
                        s.accounts = resp.accounts;
                        s.active_uid = resp.active_uid;
                        s.accounts_loaded = true;
                    }),
                    Err(_) => {
                        if let Some(user) = user_fallback {
                            cx.update_global::<AuthStore, _>(|s, _| s.user = Some(user));
                        }
                    }
                }
                this.booted = true;
                this.last_active_uid = cx.global::<AuthStore>().active_uid;

                let web = cx.global::<WebAuthState>();
                let screen = if web.is_auth_enabled() && !web.is_authenticated {
                    Screen::WebLogin
                } else if !cx.global::<AuthStore>().is_logged_in() {
                    Screen::Login
                } else {
                    Screen::Main
                };
                this.transition(screen, cx);
            });
        })
        .detach();
    }

    fn handle_event(&mut self, ev: UiEvent, cx: &mut Context<Self>) {
        match ev {
            UiEvent::WsConnection(state) => {
                self.ws_state = state;
                if state == ConnectionState::Connected {
                    self.ws.request_snapshot();
                }
                self.hub.update(cx, |_, cx| cx.emit(UiEvent::WsConnection(state)));
                cx.notify();
            }
            UiEvent::WsMessage(msg) => {
                match &msg {
                    WsServerMessage::Snapshot { downloads, uploads, transfers, folders, .. } => {
                        cx.update_global::<AuthStore, _>(|s, _| s.apply_snapshot(&msg));
                        cx.update_global::<crate::state::TaskCache, _>(|c, _| {
                            c.apply_snapshot(downloads, uploads, transfers, folders);
                        });
                        self.after_auth_change(cx);
                    }
                    WsServerMessage::Event { event } => self.apply_task_event(event, cx),
                    WsServerMessage::EventBatch { events } => {
                        for e in events {
                            self.apply_task_event(e, cx);
                        }
                    }
                    _ => {}
                }
                self.hub.update(cx, |_, cx| cx.emit(UiEvent::WsMessage(msg)));
            }
            UiEvent::WebAuthExpired => {
                cx.update_global::<WebAuthState, _>(|s, _| {
                    s.is_authenticated = false;
                    s.reset_login_flow();
                });
                self.toast(cx, ToastKind::Error, "Web 认证令牌已过期,请重新登录");
                self.transition(Screen::WebLogin, cx);
            }
            UiEvent::AccountChanged { .. } => {}
            UiEvent::Toast { kind, message } => self.toast(cx, kind, message),
            UiEvent::ActivateWindow => {
                if let Some(handle) = self.window {
                    let _ = handle.update(cx, |_, window, _| {
                        window.activate_window();
                    });
                }
            }
            UiEvent::Quit => cx.quit(),
        }
    }

    fn apply_task_event(
        &mut self,
        te: &purepan_backend::server::events::TimestampedEvent,
        cx: &mut Context<Self>,
    ) {
        if let TaskEvent::Account(acc) = &te.event {
            cx.update_global::<AuthStore, _>(|s, _| s.apply_account_event(acc));
            self.after_auth_change(cx);
        }
        // 任务事件写入 TaskCache(序列化为 {category, event} 后增量应用)
        if let Ok(v) = serde_json::to_value(&te.event) {
            if let (Some(category), Some(event)) = (v.get("category").and_then(|c| c.as_str()), v.get("event")) {
                cx.update_global::<crate::state::TaskCache, _>(|c, _| {
                    c.apply_event(te.timestamp, category, event);
                });
            }
        }
    }

    /// 账号态变化后:广播 AccountChanged(去重),登出则回登录页。
    fn after_auth_change(&mut self, cx: &mut Context<Self>) {
        let (uid, logged_in) = {
            let s = cx.global::<AuthStore>();
            (s.active_uid, s.is_logged_in())
        };
        if uid != self.last_active_uid {
            self.last_active_uid = uid;
            self.hub
                .update(cx, |_, cx| cx.emit(UiEvent::AccountChanged { new_uid: uid }));
        }
        if !logged_in && self.screen == Screen::Main {
            self.transition(Screen::Login, cx);
        }
    }

    /// 切换屏幕;登录↔主界面尺寸不同需重建窗口,Boot→Main 同尺寸直接复用。
    pub fn transition(&mut self, screen: Screen, cx: &mut Context<Self>) {
        if self.screen == screen {
            return;
        }
        let old_screen = self.screen;
        self.screen = screen;
        if old_screen == Screen::Boot && screen == Screen::Main {
            cx.notify();
            return;
        }
        let old = self.window;
        let controller = cx.entity().clone();
        cx.spawn(async move |_, cx| {
            let _ = cx.update(|cx| {
                // 先开新窗再删旧窗:GPUI 在窗口数归零时会退出应用,
                // 必须保证切换过程中始终至少有一个窗口存在。
                let (handle, _) = open_app_window(cx, screen, controller.clone());
                controller.update(cx, |c, _| c.window = Some(handle));
                if let Some(old) = old {
                    let _ = cx.update_window(old, |_, window, _| window.remove_window());
                }
            });
        })
        .detach();
    }

    /// 主导航切换(必要时先切到主屏幕)。
    pub fn navigate(&mut self, nav: Nav, cx: &mut Context<Self>) {
        self.nav = nav;
        if self.screen != Screen::Main {
            self.transition(Screen::Main, cx);
        }
        cx.notify();
    }

    /// 添加账号入口(设置页用):以 add 模式打开登录页。
    pub fn add_account(&mut self, cx: &mut Context<Self>) {
        self.login_add_mode = true;
        self.transition(Screen::Login, cx);
    }

    /// Web 认证成功后:重取账号 → 进入登录页或主界面。
    pub fn proceed_after_web_auth(&mut self, cx: &mut Context<Self>) {
        let (rt, api) = (self.rt.clone(), self.api.clone());
        cx.spawn(async move |this, cx| {
            let accounts = api::call_api(&rt, {
                let api = api.clone();
                move || async move { api.list_accounts().await }
            })
            .await;
            let user_fallback = if accounts.is_err() {
                api::call_api(&rt, {
                    let api = api.clone();
                    move || async move { api.current_user().await }
                })
                .await
                .ok()
            } else {
                None
            };
            let _ = this.update(cx, |this, cx| {
                match accounts {
                    Ok(resp) => cx.update_global::<AuthStore, _>(|s, _| {
                        s.accounts = resp.accounts;
                        s.active_uid = resp.active_uid;
                        s.accounts_loaded = true;
                    }),
                    Err(_) => {
                        if let Some(user) = user_fallback {
                            cx.update_global::<AuthStore, _>(|s, _| s.user = Some(user));
                        }
                    }
                }
                this.last_active_uid = cx.global::<AuthStore>().active_uid;
                let screen = if cx.global::<AuthStore>().is_logged_in() {
                    Screen::Main
                } else {
                    Screen::Login
                };
                this.transition(screen, cx);
            });
        })
        .detach();
    }

    pub fn toast(&self, cx: &mut Context<Self>, kind: ToastKind, message: impl Into<String>) {
        let message = message.into();
        if let Some(handle) = self.window {
            let _ = cx.update_window(handle, |_, window, cx| {
                toast::show(window, cx, kind, message.clone());
            });
        }
    }
}

/// 每窗口的 UI 壳:标题栏 + (登录页 | 侧栏+内容+状态栏)。
pub struct AppShell {
    controller: Entity<AppController>,
    views: HashMap<Nav, AnyView>,
    login: Option<Entity<LoginView>>,
    web_login: Option<Entity<WebLoginView>>,
    _subscriptions: Vec<Subscription>,
}

impl AppShell {
    fn new(
        controller: Entity<AppController>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let sub = cx.observe(&controller, |_, _, cx| cx.notify());
        Self {
            controller,
            views: HashMap::new(),
            login: None,
            web_login: None,
            _subscriptions: vec![sub],
        }
    }

    fn main_view(&mut self, nav: Nav, window: &mut Window, cx: &mut Context<Self>) -> AnyView {
        if !self.views.contains_key(&nav) {
            let view: AnyView = match nav {
                Nav::Files => {
                    let hub = self.controller.read(cx).hub.clone();
                    cx.new(|cx| crate::views::files::FilesView::new(hub, window, cx)).into()
                }
                Nav::Local => {
                    let hub = self.controller.read(cx).hub.clone();
                    cx.new(|cx| crate::views::local_files::LocalFilesView::new(hub, window, cx)).into()
                }
                Nav::Downloads => {
                    let hub = self.controller.read(cx).hub.clone();
                    cx.new(|cx| crate::views::downloads::DownloadsView::new(hub, window, cx)).into()
                }
                Nav::Uploads => {
                    let hub = self.controller.read(cx).hub.clone();
                    cx.new(|cx| crate::views::uploads::UploadsView::new(hub, window, cx)).into()
                }
                Nav::Transfers => {
                    let hub = self.controller.read(cx).hub.clone();
                    cx.new(|cx| crate::views::transfers::TransfersView::new(hub, window, cx)).into()
                }
                Nav::CloudDl => {
                    let hub = self.controller.read(cx).hub.clone();
                    cx.new(|cx| crate::views::cloud_dl::CloudDlView::new(hub, window, cx)).into()
                }
                Nav::Shares => {
                    let hub = self.controller.read(cx).hub.clone();
                    cx.new(|cx| crate::views::shares::SharesView::new(hub, window, cx)).into()
                }
                Nav::ShareSync => {
                    let hub = self.controller.read(cx).hub.clone();
                    cx.new(|cx| crate::views::share_sync::ShareSyncView::new(hub, window, cx)).into()
                }
                Nav::AutoBackup => {
                    let hub = self.controller.read(cx).hub.clone();
                    cx.new(|cx| crate::views::auto_backup::AutoBackupView::new(hub, window, cx)).into()
                }
                Nav::Settings => {
                    let hub = self.controller.read(cx).hub.clone();
                    let controller = self.controller.clone();
                    cx.new(|cx| crate::views::settings::SettingsView::new(controller, hub, window, cx)).into()
                }
                // P2 起逐页替换为真实视图
                _ => cx.new(|_| Placeholder::new(nav)).into(),
            };
            self.views.insert(nav, view);
        }
        self.views[&nav].clone()
    }
}

impl Render for AppShell {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let (screen, nav, ws_state) = {
            let c = self.controller.read(cx);
            (c.screen, c.nav, c.ws_state)
        };

        let mut root = div()
            .flex()
            .flex_col()
            .size_full()
            .bg(theme.background)
            .text_color(theme.foreground)
            .child(app_title_bar::render());

        match screen {
            Screen::Boot => {
                root = root.child(
                    div()
                        .flex_1()
                        .flex()
                        .flex_col()
                        .items_center()
                        .justify_center()
                        .gap_3()
                        .child(Spinner::new().with_size(px(32.)))
                        .child("正在启动…"),
                );
            }
            Screen::Login => {
                if self.login.is_none() {
                    let controller = self.controller.clone();
                    let add_mode = self.controller.read(cx).login_add_mode;
                    self.login = Some(cx.new(|cx| {
                        LoginView::new(controller, add_mode, window, cx)
                    }));
                }
                root = root.child(self.login.clone().unwrap());
            }
            Screen::WebLogin => {
                if self.web_login.is_none() {
                    let controller = self.controller.clone();
                    self.web_login = Some(cx.new(|cx| {
                        WebLoginView::new(controller, window, cx)
                    }));
                }
                root = root.child(self.web_login.clone().unwrap());
            }
            Screen::Main => {
                let controller = self.controller.clone();
                root = root
                    .child(
                        h_flex()
                            .flex_1()
                            .overflow_hidden()
                            .child(app_sidebar::build(nav, move |n| {
                                let controller = controller.clone();
                                move |_, _, cx| {
                                    controller.update(cx, |c, cx| c.navigate(n, cx));
                                }
                            }))
                            .child(self.main_view(nav, window, cx)),
                    )
                    .child(
                        StatusBar::new()
                            .left({
                                let connected = cx.global::<BackendHealth>().connected;
                                h_flex()
                                    .gap_1p5()
                                    .items_center()
                                    .child(
                                        div()
                                            .size(px(8.))
                                            .rounded_full()
                                            .bg(if connected { theme.success } else { theme.danger }),
                                    )
                                    .child(if connected { "后端已连接" } else { "后端未连接" })
                            })
                            .right(format!(
                                "{} · v0.1.5",
                                match ws_state {
                                    ConnectionState::Connected => "实时同步",
                                    ConnectionState::Connecting => "连接中",
                                    ConnectionState::Disconnected => "离线",
                                }
                            )),
                    );
            }
        }

        root
    }
}

/// 在 App 上下文(无实体借用)打开初始 Boot 窗口,再启动引导。
/// 窗口创建会触发首帧渲染,此时不能处于 controller.update 内,否则读控制器会 panic。
pub fn open_initial(cx: &mut App, controller: Entity<AppController>) {
    let (handle, _) = open_app_window(cx, Screen::Boot, controller.clone());
    controller.update(cx, |c, _| c.window = Some(handle));
    controller.update(cx, |c, cx| c.spawn_boot(cx));
}

fn open_app_window(
    cx: &mut App,
    screen: Screen,
    controller: Entity<AppController>,
) -> (AnyWindowHandle, Entity<AppShell>) {
    let options = match screen {
        Screen::Login | Screen::WebLogin => WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                None,
                size(px(400.), px(600.)),
                cx,
            ))),
            is_resizable: false,
            titlebar: Some(TitleBar::title_bar_options()),
            app_id: Some("com.d4go.PurePan".into()),
            ..Default::default()
        },
        _ => WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                None,
                size(px(1280.), px(860.)),
                cx,
            ))),
            window_min_size: Some(size(px(960.), px(640.))),
            app_id: Some("com.d4go.PurePan".into()),
            ..TitleBar::window_options()
        },
    };
    gpui_kit::open_window(options, cx, |window, cx| {
        window.on_window_should_close(cx, |window, _| shell::window::should_close(window));
        cx.new(|cx| AppShell::new(controller.clone(), window, cx))
    })
    .expect("无法打开窗口")
}
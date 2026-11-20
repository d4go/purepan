//! 应用控制器(常驻,跨窗口重建存活)+ 每窗口 UI 壳 AppShell。
//!
//! 控制器持有:屏幕状态机、导航、事件泵(进程内事件→Global/视图)、启动引导。
//! 窗口在 登录(400×600)↔ 主界面(1280×860) 切换时重建(Global 状态存活)。

use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

use gpui_kit::component::spinner::Spinner;
use gpui_kit::component::{Sizable as _, TitleBar, WindowExt as _};
use gpui_kit::{
    AnyView, AnyWindowHandle, App, AppContext as _, BorrowAppContext as _, Bounds, Context, Entity,
    ExternalPaths, InteractiveElement as _, IntoElement, ParentElement as _, Render, Styled as _,
    Subscription, Task, Window, WindowBounds, WindowOptions, div, px, size,
};
use purepan_backend::server::events::TaskEvent;
use purepan_backend::server::websocket::WsServerMessage;

use crate::api::{self, ApiClient};
use crate::backend_embed::BackendStatus;
use crate::bridge::{ConnectionState, EventHub, ToastKind, UiEvent};
use crate::components::dialogs::transfer_dialog::TransferDialog;
use crate::components::app_sidebar::{self, Nav, StorageStatus};
use crate::components::{app_title_bar, toast};
use crate::design;
use crate::state::{AuthStore, Backend, BackendHealth};
use crate::views::files::{FilesView, queue_upload_paths};
use crate::views::{login::LoginView, placeholder::Placeholder};
use crate::{bridge, shell};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Boot,
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
    status: tokio::sync::watch::Receiver<BackendStatus>,
    ws: crate::ws::WsHandle,
    booted: bool,
    last_active_uid: Option<u64>,
    storage_uid: Option<u64>,
    storage: StorageStatus,
    storage_task: Option<Task<()>>,
    _storage_account_subscription: Subscription,
    _pump: Task<()>,
    _clipboard: Task<()>,
}

impl AppController {
    pub fn new(cx: &mut Context<Self>, ui_rx: async_channel::Receiver<UiEvent>) -> Self {
        let b = cx.global::<Backend>();
        let rt = b.rt.clone();
        let api = b.api.clone();
        let status = b.status.clone();
        let ws = b.ws.clone();
        let hub = cx.new(|_| EventHub);

        // 事件泵:进程内事件 → Global 状态 + 广播给视图
        let pump = cx.spawn(async move |this, cx| {
            while let Ok(ev) = ui_rx.recv().await {
                if this.update(cx, |this, cx| this.handle_event(ev, cx)).is_err() {
                    break;
                }
            }
        });

        // 剪贴板监听:复制了新的百度分享链接时,直接弹出预填的转存对话框。
        // 以启动时的剪贴板为基线,只对之后的复制动作做出反应。
        let clipboard = cx.spawn(async move |this, cx| {
            let mut last = cx.update(|cx| cx.read_from_clipboard().and_then(|i| i.text()));
            loop {
                cx.background_executor().timer(Duration::from_millis(1200)).await;
                let Some(text) = cx.update(|cx| cx.read_from_clipboard().and_then(|i| i.text())) else {
                    continue;
                };
                if last.as_deref() == Some(text.as_str()) {
                    continue;
                }
                let Some((url, pwd)) = crate::util::parse_share_link(&text) else {
                    last = Some(text);
                    continue;
                };
                let opened = this.update(cx, |this, cx| {
                    if this.screen != Screen::Main {
                        return false;
                    }
                    let Some(handle) = this.window else {
                        return false;
                    };
                    let active = handle.update(cx, |_, w, _| w.is_window_active()).unwrap_or(false);
                    if !active {
                        // 窗口不活跃时不消费:切回应用后会再次检测并提示
                        return false;
                    }
                    let (url, pwd) = (url.clone(), pwd.clone());
                    let _ = handle.update(cx, |_, window, cx| {
                        let on_success: Rc<dyn Fn(&mut Window, &mut App)> = Rc::new(|_w, cx| {
                            cx.global::<Backend>().ws.request_snapshot();
                        });
                        let dlg = cx.new(|cx| TransferDialog::new(false, "/", on_success, window, cx));
                        dlg.update(cx, |d, cx| d.set_share(&url, pwd.as_deref(), window, cx));
                        window.open_dialog(cx, move |d, _, _| {
                            d.title("转存分享链接").width(px(600.)).child(dlg.clone())
                        });
                    });
                    true
                }).unwrap_or(false);
                if opened {
                    last = Some(text);
                }
            }
        });
        let storage_account_subscription = cx.observe_global::<AuthStore>(Self::sync_storage_account);

        Self {
            screen: Screen::Boot,
            nav: Nav::Files,
            login_add_mode: false,
            ws_state: ConnectionState::Disconnected,
            window: None,
            hub,
            rt,
            api,
            status,
            ws,
            booted: false,
            last_active_uid: None,
            storage_uid: None,
            storage: StorageStatus::Unavailable,
            storage_task: None,
            _storage_account_subscription: storage_account_subscription,
            _pump: pump,
            _clipboard: clipboard,
        }
    }

    /// 从托盘/程序坞激活:有窗口则聚焦,窗口已关闭则按当前屏幕重建。
    pub fn restore_window(&mut self, cx: &mut Context<Self>) {
        if let Some(handle) = self.window {
            let activated = handle
                .update(cx, |_, window, _| window.activate_window())
                .is_ok();
            if activated {
                return;
            }
        }
        let screen = if self.screen == Screen::Boot {
            Screen::Login
        } else {
            self.screen
        };
        let controller = cx.entity();
        let (handle, _) = open_app_window(cx, screen, controller);
        self.window = Some(handle);
        cx.notify();
    }

    fn sync_storage_account(&mut self, cx: &mut Context<Self>) {        let uid = cx.global::<AuthStore>().signed_in_uid();
        if uid == self.storage_uid {
            return;
        }
        self.storage_uid = uid;
        self.storage = if uid.is_some() {
            StorageStatus::Loading
        } else {
            StorageStatus::Unavailable
        };
        self.storage_task = uid.map(|uid| {
            let (rt, api) = (self.rt.clone(), self.api.clone());
            cx.spawn(async move |this, cx| {
                // 登录后后台预热可能因百度会话尚未生效而失败，后端会在容量接口
                // 内按需补预热；这里多留几次短间隔重试，让空间尽早显示出来。
                let mut startup_retries = Some([2, 4, 8, 15].into_iter());
                loop {
                    let api = api.clone();
                    let result =
                        api::call_api(&rt, move || async move { api.storage_quota(uid).await })
                            .await;
                    let (status, delay) = match result {
                        Ok(quota) => {
                            startup_retries = None;
                            (StorageStatus::Ready(quota), 60)
                        }
                        Err(_) => match startup_retries.as_mut().and_then(Iterator::next) {
                            Some(delay) => (StorageStatus::Loading, delay),
                            None => (StorageStatus::Unavailable, 30),
                        },
                    };
                    let active = this
                        .update(cx, |this, cx| {
                            if cx.global::<AuthStore>().signed_in_uid() != Some(uid) {
                                return false;
                            }
                            if this.storage != status {
                                this.storage = status;
                                cx.notify();
                            }
                            true
                        })
                        .unwrap_or(false);
                    if !active {
                        break;
                    }
                    cx.background_executor()
                        .timer(Duration::from_secs(delay))
                        .await;
                }
            })
        });
        cx.notify();
    }

    /// 开窗前同步探测登录态与主题配置(内嵌后端通常 <1s,最多等 5s),
    /// 保证第一帧窗口就是正确尺寸与配色,不会出现大框闪一下再变登录框。
    fn probe_boot(&mut self, cx: &mut Context<Self>) -> Screen {
        let mut status = self.status.clone();
        let api = self.api.clone();

        let (ready, accounts, user, theme_setting) = self.rt.block_on(async move {
            let ready = tokio::time::timeout(Duration::from_secs(5), wait_ready(&mut status))
                .await
                .unwrap_or(false);
            if !ready {
                return (false, None, None, None);
            }
            let accounts = api.list_accounts().await.ok();
            let user = if accounts.is_none() {
                api.current_user().await.ok()
            } else {
                None
            };
            let theme_setting = api.get_config().await.ok().map(|c| c.ui.theme);
            (true, accounts, user, theme_setting)
        });

        if let Some(setting) = theme_setting {
            crate::theme::apply_setting(setting, cx);
        }

        cx.update_global::<BackendHealth, _>(|h, _| h.connected = ready);
        match accounts {
            Some(resp) => {
                cx.update_global::<AuthStore, _>(|s, _| {
                    s.accounts = resp.accounts;
                    s.active_uid = resp.active_uid;
                    s.accounts_loaded = true;
                });
            }
            None => {
                if let Some(user) = user {
                    cx.update_global::<AuthStore, _>(|s, _| s.user = Some(user));
                }
            }
        }

        let screen = if !cx.global::<AuthStore>().is_logged_in() {
            Screen::Login
        } else {
            Screen::Main
        };
        self.screen = screen;
        self.booted = true;
        self.last_active_uid = cx.global::<AuthStore>().active_uid;
        screen
    }

    /// 启动引导任务:窗口已按探测定好的屏幕打开,这里继续等待后端就绪并校准状态。
    fn spawn_boot(&mut self, cx: &mut Context<Self>) {
        let (rt, api, mut status) = (self.rt.clone(), self.api.clone(), self.status.clone());
        cx.spawn(async move |this, cx| {
            // 1. 等待后端就绪(最多 30s)
            let ready = bridge::call(&rt, move || async move {
                tokio::time::timeout(Duration::from_secs(30), wait_ready(&mut status))
                    .await
                    .unwrap_or(false)
            })
            .await
            .unwrap_or(false);

            if !ready {
                let _ = this.update(cx, |this, cx| {
                    cx.update_global::<BackendHealth, _>(|h, _| h.connected = false);
                    this.booted = true;
                    this.toast(cx, ToastKind::Error, "后端初始化失败或超时,请查看日志");
                    this.transition(Screen::Login, cx);
                });
                return;
            }

            // 2. 百度账号
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

            // 3. 主题配置(探测阶段后端未就绪时,这里兜底校准一次)
            let theme_setting = api::call_api(&rt, {
                let api = api.clone();
                move || async move { api.get_config().await }
            })
            .await
            .ok()
            .map(|c| c.ui.theme);

            let plan = this.update(cx, |this, cx| {
                cx.update_global::<BackendHealth, _>(|h, _| h.connected = true);
                if let Some(setting) = theme_setting {
                    crate::theme::apply_setting(setting, cx);
                }
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

                let screen = if cx.global::<AuthStore>().is_logged_in() {
                    Screen::Main
                } else {
                    Screen::Login
                };
                if this.window.is_some() {
                    this.transition(screen, cx);
                    None
                } else {
                    // 窗口尚未创建:先落状态,引导任务按最终屏幕直接开窗
                    this.screen = screen;
                    Some(screen)
                }
            });

            if let Ok(Some(screen)) = plan {
                let Some(controller) = this.upgrade() else { return };
                let _ = cx.update(|cx| {
                    let (handle, _) = open_app_window(cx, screen, controller.clone());
                    let _ = controller.update(cx, |c, _| c.window = Some(handle));
                });
            }
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
            UiEvent::AccountChanged { .. } => {}
            UiEvent::DownloadDirChanged => {}
            UiEvent::Navigate(_) => {}
            UiEvent::Toast { kind, message } => self.toast(cx, kind, message),
            UiEvent::ActivateWindow => self.restore_window(cx),
            UiEvent::Quit => {
                if let Some(handle) = self.window {
                    let _ = handle.update(cx, |_, window, cx| {
                        shell::window::quit_with_confirm(window, cx)
                    });
                } else {
                    cx.quit();
                }
            }
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

    /// 切换屏幕;登录↔主界面尺寸不同需重建窗口。
    pub fn transition(&mut self, screen: Screen, cx: &mut Context<Self>) {
        if self.screen == screen {
            return;
        }
        let old_screen = self.screen;
        self.screen = screen;
        // 引导阶段窗口尚未创建:由引导任务按最终屏幕直接开窗
        if self.window.is_none() {
            cx.notify();
            return;
        }
        // 400×600 的屏幕之间(启动占位/登录)复用窗口,避免无谓闪烁
        let same_size = old_screen == Screen::Boot && screen == Screen::Login;
        if same_size {
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
    _subscriptions: Vec<Subscription>,
}

impl AppShell {
    fn new(
        controller: Entity<AppController>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let observe = cx.observe(&controller, |_, _, cx| cx.notify());
        // System 主题模式下跟随系统外观变化;固定亮/暗时忽略。
        let appearance_sub = window.observe_window_appearance(|window, cx| {
            crate::theme::sync_if_system(window, cx);
        });
        let hub = controller.read(cx).hub.clone();
        let nav_sub = cx.subscribe(&hub, {
            let controller = controller.clone();
            move |_, _, ev, cx| {
                if let UiEvent::Navigate(nav) = ev {
                    let nav = *nav;
                    controller.update(cx, |c, cx| c.navigate(nav, cx));
                }
            }
        });
        Self {
            controller,
            views: HashMap::new(),
            login: None,
            _subscriptions: vec![observe, nav_sub, appearance_sub],
        }
    }

    fn main_view(&mut self, nav: Nav, window: &mut Window, cx: &mut Context<Self>) -> AnyView {
        if !self.views.contains_key(&nav) {
            let view: AnyView = match nav {
                Nav::Files => {
                    let hub = self.controller.read(cx).hub.clone();
                    cx.new(|cx| FilesView::new(hub, window, cx)).into()
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

    fn on_file_drop(&mut self, files: &ExternalPaths, window: &mut Window, cx: &mut Context<Self>) {
        if files.paths().is_empty() || window.has_active_dialog(cx) {
            return;
        }
        let Some(uid) = cx.global::<AuthStore>().signed_in_uid() else {
            return;
        };
        let remote_dir = if self.controller.read(cx).nav == Nav::Files {
            let target = self
                .views
                .get(&Nav::Files)
                .cloned()
                .and_then(|view| view.downcast::<FilesView>().ok())
                .and_then(|view| view.read(cx).upload_target(cx));
            let Some(target) = target else {
                toast::show(
                    window,
                    cx,
                    ToastKind::Warning,
                    "目录正在加载，请稍后再拖入文件",
                );
                return;
            };
            target
        } else {
            "/".to_string()
        };
        queue_upload_paths(files.paths().to_vec(), remote_dir, uid, window, cx);
    }
}

impl Render for AppShell {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = design::palette(cx);
        let screen = self.controller.read(cx).screen;
        let nav = self.controller.read(cx).nav;
        // 窗口 chrome 底色:登录/启动窗为 canvas(与登录框同色);
        // 主窗为 chrome 色,顶栏/侧栏/内容区外沿连为一体,内容面板浮于其上。
        let frame_bg = match screen {
            Screen::Boot | Screen::Login => palette.canvas,
            _ => palette.chrome,
        };

        let mut root = div()
            .flex()
            .flex_col()
            .size_full()
            .bg(frame_bg)
            .text_color(palette.text)
            .child(app_title_bar::render(frame_bg));

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
            Screen::Main => {
                let controller = self.controller.clone();
                let storage = controller.read(cx).storage;
                let sidebar = app_sidebar::build(nav, &palette, storage, |nav| {
                    let controller = controller.clone();
                    move |_, _, cx| {
                        controller.update(cx, |c, cx| c.navigate(nav, cx));
                    }
                });
                let drop_background = palette.primary_soft;
                root = root.child(
                    div()
                        .id("file-drop-target")
                        .flex()
                        .flex_row()
                        .flex_1()
                        .min_h_0()
                        .overflow_hidden()
                        .drag_over::<ExternalPaths>(move |style, _, _, _| style.bg(drop_background))
                        .on_drop(cx.listener(Self::on_file_drop))
                        .child(sidebar)
                        .child(
                            div()
                                .id("content-panel")
                                .flex_1()
                                .min_w_0()
                                .mr(px(6.))
                                .mb(px(6.))
                                .overflow_hidden()
                                .bg(palette.canvas)
                                .rounded(px(12.))
                                .drag_over::<ExternalPaths>(
                                    move |style, _, _, _| style.bg(drop_background),
                                )
                                .child(self.main_view(nav, window, cx)),
                        ),
                );
            }
        }

        root
    }
}

/// 等待后端进入 Ready；Failed 或通知通道关闭视为失败。
async fn wait_ready(status: &mut tokio::sync::watch::Receiver<BackendStatus>) -> bool {
    loop {
        let current = status.borrow_and_update().clone();
        match current {
            BackendStatus::Ready(_) => return true,
            BackendStatus::Failed(_) => return false,
            BackendStatus::Starting => {
                if status.changed().await.is_err() {
                    return false;
                }
            }
        }
    }
}

/// 启动控制器:开窗前先探测登录态,直接按 400×600(登录)或主界面尺寸建窗,
/// 随后交给引导任务校准状态。窗口创建会触发首帧渲染,不能处于 controller.update 内。
pub fn open_initial(cx: &mut App, controller: Entity<AppController>) {
    let screen = controller.update(cx, |c, cx| c.probe_boot(cx));
    let (handle, _) = open_app_window(cx, screen, controller.clone());
    controller.update(cx, |c, cx| {
        c.window = Some(handle);
        c.sync_storage_account(cx);
    });
    controller.update(cx, |c, cx| c.spawn_boot(cx));
}

fn open_app_window(
    cx: &mut App,
    screen: Screen,
    controller: Entity<AppController>,
) -> (AnyWindowHandle, Entity<AppShell>) {
    let options = match screen {
        Screen::Boot | Screen::Login => WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                None,
                size(px(400.), px(600.)),
                cx,
            ))),
            is_resizable: false,
            titlebar: Some(TitleBar::title_bar_options()),
            app_id: Some("com.d4go.PurePan".into()),
            icon: Some(crate::util::app_icon()),
            ..Default::default()
        },
        _ => WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                None,
                size(px(960.), px(640.)),
                cx,
            ))),
            window_min_size: Some(size(px(960.), px(640.))),
            app_id: Some("com.d4go.PurePan".into()),
            icon: Some(crate::util::app_icon()),
            ..TitleBar::window_options()
        },
    };
    gpui_kit::open_window(options, cx, |window, cx| {
        window.on_window_should_close(cx, |window, cx| shell::window::should_close(window, cx));
        cx.new(|cx| AppShell::new(controller.clone(), window, cx))
    })
    .expect("无法打开窗口")
}

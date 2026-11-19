use std::future::Future;

use futures::channel::oneshot;
use gpui_kit::EventEmitter;
use purepan_backend::server::websocket::WsServerMessage;
use tokio::runtime::Handle;

#[derive(Debug, thiserror::Error)]
pub enum CallError {
    #[error("请求已取消")]
    Canceled,
}

/// 在 tokio 运行时上执行异步调用,返回可在 GPUI executor 上 await 的 future。
/// oneshot 通道与 executor 无关,两侧都能用。
pub fn call<F, Fut, T>(rt: &Handle, f: F) -> impl Future<Output = Result<T, CallError>>
where
    F: FnOnce() -> Fut + Send + 'static,
    Fut: Future<Output = T> + Send + 'static,
    T: Send + 'static,
{
    let (tx, rx) = oneshot::channel();
    rt.spawn(async move {
        let _ = tx.send(f().await);
    });
    async move { rx.await.map_err(|_| CallError::Canceled) }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionState {
    Disconnected,
    Connecting,
    Connected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToastKind {
    Info,
    Success,
    Warning,
    Error,
}

/// tokio 侧(WS/ApiClient)→ GPUI 侧的统一事件。
#[derive(Debug, Clone)]
pub enum UiEvent {
    WsConnection(ConnectionState),
    WsMessage(WsServerMessage),
    /// 活跃账号变化(视图据此清空重取,对应 Vue 的 multi-account:active-changed)
    AccountChanged { new_uid: Option<u64> },
    /// 下载目录已保存生效(本地文件视图据此回到新的根目录)
    DownloadDirChanged,
    /// 视图请求切换主导航(如文件页头部跳转下载)
    Navigate(crate::components::app_sidebar::Nav),
    /// 集中提示(后端断连/恢复等),由控制器弹 Notification
    Toast { kind: ToastKind, message: String },
    /// 显示/聚焦主窗口(托盘左键、二次启动)
    ActivateWindow,
    /// 退出应用(托盘退出菜单)
    Quit,
}

/// 事件分发实体:控制器泵把 UiEvent 广播给所有订阅视图。
pub struct EventHub;

impl EventEmitter<UiEvent> for EventHub {}

use std::sync::Arc;

use gpui_kit::Global;
use purepan_backend::server::AppState;
use tokio::runtime::Handle;
use tokio::sync::watch;

use crate::api::ApiClient;
use crate::bridge::UiEvent;
use crate::ws::{self, WsHandle};

/// 后端初始化状态:桌面端所有 API 调用等待进入 `Ready`。
#[derive(Clone)]
pub enum BackendStatus {
    Starting,
    Ready(AppState),
    Failed(String),
}

/// 内嵌后端:专用 tokio 运行时 + 进程内 AppState + 事件桥。
/// GPUI 的 executor 不是 tokio,所有管理器 future 必须在这个运行时上跑。
pub struct Backend {
    pub rt: Handle,
    pub api: Arc<ApiClient>,
    pub ws: WsHandle,
    pub status: watch::Receiver<BackendStatus>,
}

impl Global for Backend {}

pub fn spawn(ui: async_channel::Sender<UiEvent>) -> Backend {
    let (status_tx, status_rx) = watch::channel(BackendStatus::Starting);
    let (ws_handle, ws_cmd_rx) = ws::channel();

    let (rt_tx, rt_rx) = std::sync::mpsc::channel();
    std::thread::Builder::new()
        .name("purepan-backend".into())
        .spawn(move || {
            let rt = tokio::runtime::Builder::new_multi_thread()
                .worker_threads(2)
                .enable_all()
                .build()
                .expect("无法创建后端运行时");
            let handle = rt.handle().clone();
            rt_tx.send(handle.clone()).expect("后端运行时初始化结果发送失败");

            rt.block_on(async move {
                match purepan_backend::bootstrap().await {
                    Ok(boot) => {
                        // 日志 guard 必须与进程（运行时线程）同生命周期存活
                        let _log_guard = boot.log_guard;
                        let state = boot.state;

                        ws::spawn(&handle, state.clone(), ui, ws_cmd_rx);
                        status_tx.send_replace(BackendStatus::Ready(state));

                        // 保持运行时与日志 guard 存活；应用退出时进程整体结束
                        std::future::pending::<()>().await;
                    }
                    Err(e) => {
                        let message = format!("后端初始化失败: {e}");
                        eprintln!("{message}");
                        status_tx.send_replace(BackendStatus::Failed(message));
                        std::future::pending::<()>().await;
                    }
                }
            });
        })
        .expect("无法启动后端线程");

    let rt = rt_rx.recv().expect("后端运行时初始化失败");
    let api = Arc::new(ApiClient::new(status_rx.clone()));
    Backend {
        rt,
        api,
        ws: ws_handle,
        status: status_rx,
    }
}

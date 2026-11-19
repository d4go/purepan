//! 进程内事件桥:直接订阅后端 WsManager,替代原 WebSocket 客户端。
//!
//! 订阅/退订命令经 async_channel 送到桥接任务;事件统一转成 `UiEvent::WsMessage`
//! 交给 GPUI 泵。连接即注册,不存在网络断连/重连。

use async_channel::{Receiver, Sender};
use purepan_backend::server::websocket::inproc;
use purepan_backend::server::AppState;
use tokio::runtime::Handle;

use crate::bridge::{ConnectionState, UiEvent};

/// 后端只向订阅了对应 topic 的连接推送任务事件;`*` 覆盖下载/上传/转存/
/// 文件夹/离线/账号等普通事件,`backup:*` 单独放开备份任务隔离。
const DEFAULT_SUBSCRIPTIONS: [&str; 2] = ["*", "backup:*"];

#[derive(Debug, Clone)]
pub enum WsCommand {
    Subscribe(Vec<String>),
    Unsubscribe(Vec<String>),
    RequestSnapshot,
}

/// 事件控制句柄(存于 Backend Global,视图用它订阅话题/请求快照)。
#[derive(Clone)]
pub struct WsHandle {
    cmd: Sender<WsCommand>,
}

impl WsHandle {
    pub fn subscribe(&self, topics: Vec<String>) {
        let _ = self.cmd.try_send(WsCommand::Subscribe(topics));
    }

    pub fn unsubscribe(&self, topics: Vec<String>) {
        let _ = self.cmd.try_send(WsCommand::Unsubscribe(topics));
    }

    pub fn request_snapshot(&self) {
        let _ = self.cmd.try_send(WsCommand::RequestSnapshot);
    }
}

/// 创建句柄与命令通道;桥接任务在后端初始化完成后由 `spawn` 启动。
pub fn channel() -> (WsHandle, Receiver<WsCommand>) {
    let (tx, rx) = async_channel::unbounded();
    (WsHandle { cmd: tx }, rx)
}

pub fn spawn(
    rt: &Handle,
    state: AppState,
    ui: Sender<UiEvent>,
    cmd_rx: Receiver<WsCommand>,
) {
    rt.spawn(async move {
        let _ = ui.send(UiEvent::WsConnection(ConnectionState::Connecting)).await;
        let subscriptions = DEFAULT_SUBSCRIPTIONS
            .iter()
            .map(|s| s.to_string())
            .collect();
        let stream = inproc::connect(&state, subscriptions).await;
        let _ = ui.send(UiEvent::WsConnection(ConnectionState::Connected)).await;

        let connection_id = stream.connection_id.clone();
        let mut messages = stream.receiver;
        loop {
            tokio::select! {
                msg = messages.recv() => {
                    match msg {
                        Some(msg) => {
                            let _ = ui.send(UiEvent::WsMessage(msg)).await;
                        }
                        None => break,
                    }
                }
                cmd = cmd_rx.recv() => {
                    if let Ok(cmd) = cmd {
                        match cmd {
                            WsCommand::Subscribe(topics) => {
                                inproc::subscribe(&state, &connection_id, topics).await;
                            }
                            WsCommand::Unsubscribe(topics) => {
                                inproc::unsubscribe(&state, &connection_id, topics).await;
                            }
                            WsCommand::RequestSnapshot => {
                                inproc::request_snapshot(&state, &connection_id).await;
                            }
                        }
                    }
                }
            }
        }

        inproc::disconnect(&state, &connection_id).await;
        let _ = ui.send(UiEvent::WsConnection(ConnectionState::Disconnected)).await;
    });
}

//! WebSocket 客户端(移植 frontend/src/utils/websocket.ts)。
//!
//! 在 tokio 运行时上跑监督循环:连接 → 恢复订阅 + 请求快照 → 心跳/收发;
//! 断开后按 1/2/4/8/16/30s 退避重连。事件经 UiEvent 通道送到 GPUI 泵。
//! 消息类型直接复用后端定义(同时 derive Serialize/Deserialize,契约零漂移)。

use std::collections::HashSet;
use std::time::{Duration, Instant};

use futures::{SinkExt, StreamExt};
use purepan_backend::server::websocket::{WsClientMessage, WsServerMessage};
use tokio::runtime::Handle;
use tokio_tungstenite::tungstenite::Message;

use crate::bridge::{ConnectionState, UiEvent};

const RECONNECT_DELAYS_MS: [u64; 6] = [1000, 2000, 4000, 8000, 16000, 30000];
const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(30);
const HEARTBEAT_TIMEOUT: Duration = Duration::from_secs(60);

/// 后端只向订阅了对应 topic 的连接推送任务事件;`*` 覆盖下载/上传/转存/
/// 文件夹/离线/账号等普通事件,`backup:*` 单独放开备份任务隔离。
const DEFAULT_SUBSCRIPTIONS: [&str; 2] = ["*", "backup:*"];

#[derive(Debug, Clone)]
pub enum WsCommand {
    Subscribe(Vec<String>),
    Unsubscribe(Vec<String>),
    RequestSnapshot,
}

/// WS 控制句柄(存于 Backend Global,视图用它订阅话题)。
#[derive(Clone)]
pub struct WsHandle {
    cmd: async_channel::Sender<WsCommand>,
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

pub fn spawn(rt: &Handle, url: String, ui: async_channel::Sender<UiEvent>) -> WsHandle {
    let (cmd_tx, cmd_rx) = async_channel::unbounded::<WsCommand>();
    rt.spawn(async move {
        let mut subs: HashSet<String> = DEFAULT_SUBSCRIPTIONS.iter().map(|s| s.to_string()).collect();
        let mut attempt: usize = 0;
        loop {
            // 连接前先消化积压的订阅命令,保证重连后订阅是最新的
            while let Ok(cmd) = cmd_rx.try_recv() {
                apply_cmd(&mut subs, cmd);
            }

            let _ = ui.send(UiEvent::WsConnection(ConnectionState::Connecting)).await;
            match tokio_tungstenite::connect_async(&url).await {
                Ok((stream, _)) => {
                    attempt = 0;
                    let _ = ui.send(UiEvent::WsConnection(ConnectionState::Connected)).await;
                    let (mut sink, mut source) = stream.split();

                    if !subs.is_empty() {
                        let msg = WsClientMessage::Subscribe {
                            subscriptions: subs.iter().cloned().collect(),
                        };
                        let _ = sink.send(text(&msg)).await;
                    }
                    let _ = sink.send(text(&WsClientMessage::RequestSnapshot)).await;

                    let mut last_pong = Instant::now();
                    let mut heartbeat = tokio::time::interval(HEARTBEAT_INTERVAL);
                    heartbeat.tick().await; // 消费掉立即触发的首次 tick
                    let mut closed = false;

                    while !closed {
                        tokio::select! {
                            incoming = source.next() => {
                                match incoming {
                                    Some(Ok(Message::Text(payload))) => {
                                        let payload = payload.to_string();
                                        match serde_json::from_str::<WsServerMessage>(&payload) {
                                            Ok(WsServerMessage::Pong { .. }) => {
                                                last_pong = Instant::now();
                                            }
                                            Ok(msg) => {
                                                let _ = ui.send(UiEvent::WsMessage(msg)).await;
                                            }
                                            Err(e) => {
                                                tracing::warn!("WS 消息解析失败: {e}");
                                            }
                                        }
                                    }
                                    Some(Ok(Message::Ping(p))) => {
                                        let _ = sink.send(Message::Pong(p)).await;
                                    }
                                    Some(Ok(Message::Close(_))) | None => closed = true,
                                    Some(Err(e)) => {
                                        tracing::warn!("WS 连接错误: {e}");
                                        closed = true;
                                    }
                                    _ => {}
                                }
                            }
                            cmd = cmd_rx.recv() => {
                                if let Ok(cmd) = cmd {
                                    let wire = match &cmd {
                                        WsCommand::Subscribe(t) => Some(WsClientMessage::Subscribe { subscriptions: t.clone() }),
                                        WsCommand::Unsubscribe(t) => Some(WsClientMessage::Unsubscribe { subscriptions: t.clone() }),
                                        WsCommand::RequestSnapshot => Some(WsClientMessage::RequestSnapshot),
                                    };
                                    apply_cmd(&mut subs, cmd);
                                    if let Some(w) = wire {
                                        let _ = sink.send(text(&w)).await;
                                    }
                                }
                            }
                            _ = heartbeat.tick() => {
                                if last_pong.elapsed() > HEARTBEAT_TIMEOUT {
                                    tracing::warn!("WS 心跳超时,重连");
                                    closed = true;
                                } else {
                                    let ping = WsClientMessage::Ping {
                                        timestamp: chrono::Utc::now().timestamp_millis(),
                                    };
                                    let _ = sink.send(text(&ping)).await;
                                }
                            }
                        }
                    }
                    let _ = ui.send(UiEvent::WsConnection(ConnectionState::Disconnected)).await;
                }
                Err(e) => {
                    tracing::warn!("WS 连接失败: {e}");
                    let _ = ui.send(UiEvent::WsConnection(ConnectionState::Disconnected)).await;
                }
            }

            let delay = RECONNECT_DELAYS_MS[attempt.min(RECONNECT_DELAYS_MS.len() - 1)];
            attempt += 1;
            tokio::time::sleep(Duration::from_millis(delay)).await;
        }
    });
    WsHandle { cmd: cmd_tx }
}

fn apply_cmd(subs: &mut HashSet<String>, cmd: WsCommand) {
    match cmd {
        WsCommand::Subscribe(topics) => {
            subs.extend(topics);
        }
        WsCommand::Unsubscribe(topics) => {
            for t in topics {
                subs.remove(&t);
            }
        }
        WsCommand::RequestSnapshot => {}
    }
}

fn text(msg: &WsClientMessage) -> Message {
    Message::Text(serde_json::to_string(msg).unwrap_or_default().into())
}
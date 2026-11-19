//! 进程内事件订阅
//!
//! 桌面端与后端同进程，直接向 `WsManager` 注册连接并消费事件流，
//! 不再建立 TCP WebSocket。订阅/退订语义与 `handler.rs` 的 WS 分支保持一致
//! （包括 cloud_dl monitor 的订阅者计数），避免实时事件断流或计数泄漏。

use tokio::sync::mpsc;
use tracing::{debug, warn};
use uuid::Uuid;

use super::message::WsServerMessage;
use crate::server::AppState;

/// 进程内订阅句柄：连接 ID + 事件接收器。
pub struct InprocEventStream {
    pub connection_id: String,
    pub receiver: mpsc::UnboundedReceiver<WsServerMessage>,
}

fn is_cloud_dl(topic: &str) -> bool {
    topic == "cloud_dl" || topic.starts_with("cloud_dl:")
}

fn has_cloud_dl_subscription(topics: &[String]) -> bool {
    topics.iter().any(|s| is_cloud_dl(s))
}

/// 注册连接、应用订阅并推送初始快照。
pub async fn connect(state: &AppState, subscriptions: Vec<String>) -> InprocEventStream {
    let connection_id = Uuid::new_v4().to_string();
    let receiver = state.ws_manager.register(connection_id.clone());

    state
        .ws_manager
        .send_to(&connection_id, WsServerMessage::connected(connection_id.clone()));
    subscribe(state, &connection_id, subscriptions).await;
    request_snapshot(state, &connection_id).await;

    InprocEventStream {
        connection_id,
        receiver,
    }
}

pub async fn subscribe(state: &AppState, connection_id: &str, subscriptions: Vec<String>) {
    let subscribing_cloud_dl = has_cloud_dl_subscription(&subscriptions);
    let was_subscribed_cloud_dl = has_cloud_dl_subscription(
        &state.ws_manager.get_subscriptions(connection_id),
    );

    state.ws_manager.subscribe(connection_id, subscriptions);

    if subscribing_cloud_dl && !was_subscribed_cloud_dl {
        if let Some(active_uid) = *state.active_uid.read().await {
            if !state.cloud_dl_monitors.contains_key(&active_uid) {
                if let Err(e) = state.ensure_client_for_uid(active_uid).await {
                    warn!(
                        "cloud_dl 订阅前 ensure_client_for_uid 失败: uid={}, err={:?}",
                        active_uid.raw(),
                        e
                    );
                } else {
                    state.init_cloud_dl_monitor_for(active_uid).await;
                }
            }
            if let Some(monitor) = state.cloud_dl_monitor_for(active_uid) {
                monitor.add_subscriber();
                state
                    .cloud_dl_ws_subscribers
                    .insert(connection_id.to_string(), active_uid);
                debug!(
                    "cloud_dl 订阅者增加: connection={}, uid={}",
                    connection_id,
                    active_uid.raw()
                );
            }
        }
    }

    let current = state.ws_manager.get_subscriptions(connection_id);
    state
        .ws_manager
        .send_to(connection_id, WsServerMessage::subscribe_success(current));
}

pub async fn unsubscribe(state: &AppState, connection_id: &str, subscriptions: Vec<String>) {
    let unsubscribing_cloud_dl = has_cloud_dl_subscription(&subscriptions);

    state.ws_manager.unsubscribe(connection_id, subscriptions);

    if unsubscribing_cloud_dl {
        let remaining = state.ws_manager.get_subscriptions(connection_id);
        if !has_cloud_dl_subscription(&remaining) {
            if let Some((_, recorded_uid)) = state.cloud_dl_ws_subscribers.remove(connection_id) {
                if let Some(monitor) = state.cloud_dl_monitor_for(recorded_uid) {
                    monitor.remove_subscriber();
                    debug!(
                        "cloud_dl 订阅者减少: connection={}, uid={}",
                        connection_id,
                        recorded_uid.raw()
                    );
                }
            }
        }
    }

    let current = state.ws_manager.get_subscriptions(connection_id);
    state
        .ws_manager
        .send_to(connection_id, WsServerMessage::unsubscribe_success(current));
}

pub async fn request_snapshot(state: &AppState, connection_id: &str) {
    let snapshot = super::handler::get_snapshot(state).await;
    state.ws_manager.send_to(connection_id, snapshot);
}

/// 断开连接：与 WS handler 关闭路径一致，释放 cloud_dl 计数后注销连接。
pub async fn disconnect(state: &AppState, connection_id: &str) {
    let subscriptions = state.ws_manager.get_subscriptions(connection_id);
    if has_cloud_dl_subscription(&subscriptions) {
        if let Some((_, recorded_uid)) = state.cloud_dl_ws_subscribers.remove(connection_id) {
            if let Some(monitor) = state.cloud_dl_monitor_for(recorded_uid) {
                monitor.remove_subscriber();
            }
        }
    }
    state.ws_manager.unregister(connection_id);
}

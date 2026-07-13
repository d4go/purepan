//! Tauri IPC 代理层
//!
//! 提供两个核心能力：
//! 1. `ipc_http` — 将前端的 invoke 调用代理为 HTTP 请求发往内嵌 Axum 服务器
//! 2. `ipc_ws_*` — WebSocket 桥接：Rust 侧连接本地 WS，将消息以 Tauri 事件转发给前端
//!
//! 后端业务逻辑零修改：Axum 服务器照常运行，本模块仅做协议桥接。

use std::collections::HashMap;
use std::sync::Arc;

use futures_util::{SinkExt, StreamExt};
use serde::Serialize;
use tauri::{AppHandle, Emitter, State};
use tokio::sync::Mutex;
use tokio_tungstenite::{
    connect_async,
    tungstenite::Message,
    MaybeTlsStream, WebSocketStream,
};

/// 内嵌 Axum 服务器的基础地址
const BACKEND_BASE_URL: &str = "http://127.0.0.1:18888";
const BACKEND_WS_URL: &str = "ws://127.0.0.1:18888/api/v1/ws";

// ============================================================
// HTTP 代理
// ============================================================

/// IPC HTTP 响应（返回给前端 invoke 调用）
#[derive(Serialize)]
pub struct IpcHttpResponse {
    pub status: u16,
    pub body: serde_json::Value,
}

/// HTTP 代理命令
///
/// 前端通过 `invoke('ipc_http', { method, path, body, headers })` 调用。
/// `path` 是不含 host 的路径，如 `/api/v1/auth/qrcode/generate?sign=xxx`。
/// 返回 `{ status, body }`；连接失败时返回 Err（前端 adapter 转为无 response 的网络错误）。
#[tauri::command]
pub async fn ipc_http(
    method: String,
    path: String,
    body: Option<serde_json::Value>,
    headers: Option<HashMap<String, String>>,
) -> Result<IpcHttpResponse, String> {
    let url = format!("{}{}", BACKEND_BASE_URL, path);

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .build()
        .map_err(|e| format!("创建 HTTP 客户端失败: {}", e))?;

    let mut req = match method.to_uppercase().as_str() {
        "GET" => client.get(&url),
        "POST" => client.post(&url),
        "PUT" => client.put(&url),
        "DELETE" => client.delete(&url),
        "PATCH" => client.patch(&url),
        other => return Err(format!("不支持的 HTTP 方法: {}", other)),
    };

    // 转发请求头
    let has_body = body.is_some();
    if let Some(hdrs) = &headers {
        for (k, v) in hdrs {
            // 跳过 host 头，避免与目标服务器冲突
            if k.eq_ignore_ascii_case("host") {
                continue;
            }
            // 有 body 时跳过 Content-Type，由 req.json() 统一设置，避免重复头
            if has_body && k.eq_ignore_ascii_case("content-type") {
                continue;
            }
            req = req.header(k, v);
        }
    }

    // 发送请求体（JSON）
    if let Some(b) = body {
        req = req.json(&b);
    }

    let response = req.send().await.map_err(|e| {
        // 连接失败：前端 adapter 将此转为无 response 的 error，触发 isBackendDownError
        format!("后端连接失败: {}", e)
    })?;

    let status = response.status().as_u16();

    // 尝试解析为 JSON，失败则作为字符串
    let body_text = response.text().await.unwrap_or_default();
    let body_json: serde_json::Value =
        serde_json::from_str(&body_text).unwrap_or(serde_json::Value::String(body_text));

    Ok(IpcHttpResponse { status, body: body_json })
}

// ============================================================
// WebSocket 桥接
// ============================================================

/// WS 写端的类型别名
type WsSink = futures_util::stream::SplitSink<
    WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>,
    Message,
>;

/// WS 桥接共享状态
#[derive(Default)]
pub struct WsBridgeState {
    /// WS 写端（None 表示未连接）
    sender: Arc<Mutex<Option<WsSink>>>,
}

/// WS 桥接连接命令
///
/// 连接本地 Axum WS 服务，spawn 读循环将每条消息通过 `ws://message` 事件转发。
/// 连接/断开时通过 `ws://state` 事件通知前端。
#[tauri::command]
pub async fn ipc_ws_connect(
    app: AppHandle,
    state: State<'_, WsBridgeState>,
) -> Result<(), String> {
    // 如果已有连接，先关闭旧的
    {
        let mut guard = state.sender.lock().await;
        if let Some(mut old_sink) = guard.take() {
            let _ = old_sink.close().await;
        }
    }

    // 连接 WS
    let (ws_stream, _response) =
        connect_async(BACKEND_WS_URL)
            .await
            .map_err(|e| format!("WS 连接失败: {}", e))?;

    let (sink, mut stream) = ws_stream.split();

    // 存储写端
    {
        let mut guard = state.sender.lock().await;
        *guard = Some(sink);
    }

    // 通知前端：已连接
    let _ = app.emit("ws://state", "connected");

    // spawn 读循环
    let sender_clone = Arc::clone(&state.sender);
    let app_clone = app.clone();
    tokio::spawn(async move {
        while let Some(msg_result) = stream.next().await {
            match msg_result {
                Ok(Message::Text(text)) => {
                    // 将 WS 文本消息转发给前端
                    let _ = app_clone.emit("ws://message", text);
                }
                Ok(Message::Binary(data)) => {
                    // 二进制消息转为文本尝试转发
                    if let Ok(text) = String::from_utf8(data.to_vec()) {
                        let _ = app_clone.emit("ws://message", text);
                    }
                }
                Ok(Message::Ping(_)) | Ok(Message::Pong(_)) => {
                    // 心跳帧由 tungstenite 自动处理
                }
                Ok(Message::Close(_)) | Err(_) => {
                    // 连接关闭或出错
                    let mut guard = sender_clone.lock().await;
                    *guard = None; // 清空写端
                    let _ = app_clone.emit("ws://state", "disconnected");
                    break;
                }
                Ok(Message::Frame(_)) => {
                    // 原始帧，忽略
                }
            }
        }

        // 循环结束后确保通知断开
        let mut guard = sender_clone.lock().await;
        *guard = None;
        let _ = app_clone.emit("ws://state", "disconnected");
    });

    Ok(())
}

/// WS 发送消息命令
///
/// 前端通过 `invoke('ipc_ws_send', { message })` 发送消息（JSON 字符串）。
#[tauri::command]
pub async fn ipc_ws_send(
    state: State<'_, WsBridgeState>,
    message: String,
) -> Result<(), String> {
    let mut guard = state.sender.lock().await;
    if let Some(sink) = guard.as_mut() {
        sink.send(Message::Text(message))
            .await
            .map_err(|e| format!("WS 发送失败: {}", e))?;
        Ok(())
    } else {
        Err("WS 未连接".to_string())
    }
}

/// WS 断开连接命令
#[tauri::command]
pub async fn ipc_ws_disconnect(state: State<'_, WsBridgeState>) -> Result<(), String> {
    let mut guard = state.sender.lock().await;
    if let Some(mut sink) = guard.take() {
        let _ = sink.close().await;
    }
    Ok(())
}

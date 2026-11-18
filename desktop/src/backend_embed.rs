use std::sync::Arc;
use std::time::Duration;

use gpui_kit::Global;
use tokio::runtime::Handle;

use crate::api::ApiClient;
use crate::bridge::UiEvent;
use crate::ws::{self, WsHandle};

/// 内嵌后端:专用 tokio 运行时线程 + HTTP 客户端 + API 客户端 + WS 句柄。
/// GPUI 的 executor 不是 tokio,所有 reqwest/WS/axum future 必须在这个运行时上跑。
pub struct Backend {
    pub rt: Handle,
    pub http: reqwest::Client,
    /// 形如 http://127.0.0.1:18888
    pub base_url: String,
    pub api: Arc<ApiClient>,
    pub ws: WsHandle,
}

impl Global for Backend {}

impl Backend {
    pub fn api_url(&self, path: &str) -> String {
        format!("{}/api/v1{}", self.base_url, path)
    }
}

pub fn spawn(ui: async_channel::Sender<UiEvent>) -> Backend {
    let (tx, rx) = std::sync::mpsc::channel();
    let ui_for_thread = ui.clone();
    std::thread::Builder::new()
        .name("purepan-backend".into())
        .spawn(move || {
            let rt = tokio::runtime::Builder::new_multi_thread()
                .worker_threads(2)
                .enable_all()
                .build()
                .expect("无法创建后端运行时");
            // reqwest Client 构建需要 tokio reactor 上下文
            let (http, base_url) = {
                let _guard = rt.enter();
                let http = reqwest::Client::builder()
                    .timeout(Duration::from_secs(30))
                    .build()
                    .expect("无法创建 HTTP 客户端");
                (http, read_base_url())
            };
            let api = Arc::new(ApiClient::new(http.clone(), &base_url, ui_for_thread.clone()));
            let ws_url = format!("{}/api/v1/ws", base_url.replace("http://", "ws://"));
            let ws = ws::spawn(rt.handle(), ws_url, ui_for_thread);
            tx.send((rt.handle().clone(), http, base_url, api, ws))
                .expect("后端运行时初始化结果发送失败");
            rt.block_on(async {
                if let Err(e) = purepan_backend::start_server().await {
                    eprintln!("内嵌服务器错误: {e}");
                }
            });
        })
        .expect("无法启动后端线程");

    let (rt, http, base_url, api, ws) = rx.recv().expect("后端运行时初始化失败");
    Backend {
        rt,
        http,
        base_url,
        api,
        ws,
    }
}

/// 从 config/app.toml 读 [server] host/port(与后端 start_server 同一份配置)。
fn read_base_url() -> String {
    let (host, port) = std::fs::read_to_string("config/app.toml")
        .ok()
        .and_then(|s| s.parse::<toml::Value>().ok())
        .and_then(|v| v.get("server").cloned())
        .map(|s| {
            let host = s
                .get("host")
                .and_then(|v| v.as_str())
                .unwrap_or("127.0.0.1")
                .to_string();
            let port = s.get("port").and_then(|v| v.as_integer()).unwrap_or(18888);
            (host, port)
        })
        .unwrap_or(("127.0.0.1".into(), 18888));
    let host = match host.as_str() {
        "0.0.0.0" | "::" | "[::]" => "127.0.0.1".to_string(),
        _ => host,
    };
    format!("http://{host}:{port}")
}
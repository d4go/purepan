pub mod accounts;
pub mod auth;
pub mod autobackup;
pub mod cloud_dl;
pub mod config;
pub mod download;
pub mod dto;
pub mod file;
pub mod filesystem;
pub mod local_files;
pub mod share;
pub mod share_sync;
pub mod transfer;
pub mod upload;
pub mod web_auth;

use std::path::PathBuf;
use std::sync::Arc;

use reqwest::Method;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

use crate::bridge::UiEvent;

/// 视图侧便捷入口:在 tokio 上执行 API 调用并把取消错误归一为 ApiError。
pub async fn call_api<F, Fut, T>(rt: &tokio::runtime::Handle, f: F) -> Result<T, ApiError>
where
    F: FnOnce() -> Fut + Send + 'static,
    Fut: std::future::Future<Output = Result<T, ApiError>> + Send + 'static,
    T: Send + 'static,
{
    crate::bridge::call(rt, f)
        .await
        .unwrap_or(Err(ApiError::Canceled))
}

#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    /// 业务错误(信封 code != 0 或 HTTP 非 2xx),携带错误码供调用方分支(如转存 need-password)
    #[error("{message}")]
    Business { code: i64, message: String },
    #[error("{0}")]
    Network(String),
    #[error("Web 认证令牌已过期,请重新登录")]
    WebAuthExpired,
    #[error("响应解析失败: {0}")]
    Decode(String),
    #[error("请求已取消")]
    Canceled,
}

impl ApiError {
    pub fn message(&self) -> String {
        self.to_string()
    }
}

/// Web 认证令牌(替代 Vue 的 localStorage)。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Tokens {
    pub access_token: Option<String>,
    pub refresh_token: Option<String>,
    pub access_expires_at: Option<i64>,
    pub refresh_expires_at: Option<i64>,
}

#[derive(Deserialize)]
struct Envelope<T> {
    code: i64,
    #[serde(default)]
    message: String,
    data: Option<T>,
}

/// HTTP API 客户端:信封解包、419 单飞刷新、令牌持久化。
/// 所有方法都是 async,必须经 bridge::call 在 tokio 运行时上执行。
pub struct ApiClient {
    http: reqwest::Client,
    /// 形如 http://127.0.0.1:18888/api/v1
    base: String,
    tokens: Arc<Mutex<Tokens>>,
    refresh_lock: Arc<Mutex<()>>,
    ui: async_channel::Sender<UiEvent>,
    token_file: PathBuf,
}

impl ApiClient {
    pub fn new(
        http: reqwest::Client,
        base_url: &str,
        ui: async_channel::Sender<UiEvent>,
    ) -> Self {
        let token_file = PathBuf::from("web_auth_tokens.json");
        let tokens = std::fs::read_to_string(&token_file)
            .ok()
            .and_then(|s| serde_json::from_str::<Tokens>(&s).ok())
            .unwrap_or_default();
        Self {
            http,
            base: format!("{base_url}/api/v1"),
            tokens: Arc::new(Mutex::new(tokens)),
            refresh_lock: Arc::new(Mutex::new(())),
            ui,
            token_file,
        }
    }

    pub fn base(&self) -> &str {
        &self.base
    }

    pub async fn tokens_snapshot(&self) -> Tokens {
        self.tokens.lock().await.clone()
    }

    /// 登录成功后写入令牌(并持久化)。
    pub async fn set_tokens(&self, tokens: Tokens) {
        *self.tokens.lock().await = tokens.clone();
        self.persist_tokens(&tokens);
    }

    pub async fn clear_tokens(&self) {
        let mut t = self.tokens.lock().await;
        *t = Tokens::default();
        let _ = std::fs::remove_file(&self.token_file);
    }

    fn persist_tokens(&self, tokens: &Tokens) {
        if let Ok(s) = serde_json::to_string(tokens) {
            let _ = std::fs::write(&self.token_file, s);
        }
    }

    /// 标准信封请求:{code,message,data},code != 0 → Business 错误。
    pub async fn request<T: DeserializeOwned>(
        &self,
        method: Method,
        path: &str,
        body: Option<serde_json::Value>,
    ) -> Result<T, ApiError> {
        let (data, _) = self.request_with_message(method, path, body).await?;
        Ok(data)
    }

    /// 同 request,但额外返回信封 message(cookie 登录的预热提示等需要)。
    pub async fn request_with_message<T: DeserializeOwned>(
        &self,
        method: Method,
        path: &str,
        body: Option<serde_json::Value>,
    ) -> Result<(T, String), ApiError> {
        let text = self.send_authorized(method, path, body).await?;
        let env: Envelope<T> =
            serde_json::from_str(&text).map_err(|e| ApiError::Decode(e.to_string()))?;
        if env.code != 0 {
            return Err(ApiError::Business {
                code: env.code,
                message: if env.message.is_empty() {
                    "请求失败".into()
                } else {
                    env.message
                },
            });
        }
        let data = env
            .data
            .ok_or_else(|| ApiError::Decode("响应缺少 data 字段".into()))?;
        Ok((data, env.message))
    }

    /// 原始请求:响应体直接是 T(不包信封),用于 web-auth / accounts 等。
    pub async fn request_raw<T: DeserializeOwned>(
        &self,
        method: Method,
        path: &str,
        body: Option<serde_json::Value>,
    ) -> Result<T, ApiError> {
        let text = self.send_authorized(method, path, body).await?;
        serde_json::from_str(&text).map_err(|e| ApiError::Decode(e.to_string()))
    }

    /// 兼容请求:优先按裸 JSON 解析 T;若响应带 code 字段则按信封解包。
    /// (accounts 接口两种形态都可能出现,移植自 accounts.ts 的 fallback 解构)
    pub async fn request_flex<T: DeserializeOwned>(
        &self,
        method: Method,
        path: &str,
        body: Option<serde_json::Value>,
    ) -> Result<T, ApiError> {
        let text = self.send_authorized(method, path, body).await?;
        let value: serde_json::Value =
            serde_json::from_str(&text).map_err(|e| ApiError::Decode(e.to_string()))?;
        if value.get("code").is_some() {
            let env: Envelope<T> =
                serde_json::from_value(value).map_err(|e| ApiError::Decode(e.to_string()))?;
            if env.code != 0 {
                return Err(ApiError::Business {
                    code: env.code,
                    message: env.message,
                });
            }
            return env
                .data
                .ok_or_else(|| ApiError::Decode("响应缺少 data 字段".into()));
        }
        serde_json::from_value(value).map_err(|e| ApiError::Decode(e.to_string()))
    }

    /// {success,data,error} 信封(autobackup / share-sync 专用)。
    pub async fn request_success<T: DeserializeOwned>(
        &self,
        method: Method,
        path: &str,
        body: Option<serde_json::Value>,
    ) -> Result<T, ApiError> {
        #[derive(Deserialize)]
        struct SuccessEnv<T> {
            success: bool,
            data: Option<T>,
            error: Option<String>,
        }
        let text = self.send_authorized(method, path, body).await?;
        let env: SuccessEnv<T> =
            serde_json::from_str(&text).map_err(|e| ApiError::Decode(e.to_string()))?;
        if !env.success {
            return Err(ApiError::Business {
                code: -1,
                message: env.error.unwrap_or_else(|| "操作失败".into()),
            });
        }
        env.data
            .ok_or_else(|| ApiError::Decode("响应缺少 data 字段".into()))
    }

    /// 长轮询请求(copy/move/rename):单次超时 360s,容纳后端 taskquery 多次轮询。
    /// 移植 file.ts 的 longPollClient(timeout 360_000, showErrorMessage false)。
    pub async fn request_long<T: DeserializeOwned>(
        &self,
        method: Method,
        path: &str,
        body: Option<serde_json::Value>,
    ) -> Result<T, ApiError> {
        let text = self
            .send_authorized_inner(method, path, body, Some(std::time::Duration::from_secs(360)))
            .await?;
        let env: Envelope<T> =
            serde_json::from_str(&text).map_err(|e| ApiError::Decode(e.to_string()))?;
        if env.code != 0 {
            return Err(ApiError::Business {
                code: env.code,
                message: env.message,
            });
        }
        env.data
            .ok_or_else(|| ApiError::Decode("响应缺少 data 字段".into()))
    }

    /// 发送带 Bearer 令牌的请求;419 时单飞刷新后重试一次。
    /// 返回原始响应文本;非 2xx 转为 Business 错误(提取 message/error)。
    async fn send_authorized(
        &self,
        method: Method,
        path: &str,
        body: Option<serde_json::Value>,
    ) -> Result<String, ApiError> {
        self.send_authorized_inner(method, path, body, None).await
    }

    async fn send_authorized_inner(
        &self,
        method: Method,
        path: &str,
        body: Option<serde_json::Value>,
        timeout: Option<std::time::Duration>,
    ) -> Result<String, ApiError> {
        let url = format!("{}{}", self.base, path);
        let mut retried = false;
        loop {
            let token = self.tokens.lock().await.access_token.clone();
            let mut req = self.http.request(method.clone(), &url);
            if let Some(t) = &token {
                req = req.bearer_auth(t);
            }
            if let Some(t) = timeout {
                req = req.timeout(t);
            }
            if let Some(b) = &body {
                req = req.json(b);
            }
            let resp = req
                .send()
                .await
                .map_err(|e| ApiError::Network(e.to_string()))?;
            let status = resp.status();
            if status.as_u16() == 419 && !retried {
                retried = true;
                self.refresh_single_flight(token).await?;
                continue;
            }
            let text = resp
                .text()
                .await
                .map_err(|e| ApiError::Network(e.to_string()))?;
            if !status.is_success() {
                let message = serde_json::from_str::<serde_json::Value>(&text)
                    .ok()
                    .and_then(|v| {
                        v.get("message")
                            .or_else(|| v.get("error"))
                            .and_then(|m| m.as_str())
                            .map(String::from)
                    })
                    .unwrap_or_else(|| format!("请求失败 ({})", status.as_u16()));
                return Err(ApiError::Business {
                    code: status.as_u16() as i64,
                    message,
                });
            }
            return Ok(text);
        }
    }

    /// 419 单飞刷新:拿到锁后若令牌已被其他请求刷新则直接复用;
    /// 否则调 /web-auth/refresh,失败清令牌并通知 UI 跳 Web 登录。
    async fn refresh_single_flight(&self, failed_token: Option<String>) -> Result<(), ApiError> {
        let _guard = self.refresh_lock.lock().await;
        let current = self.tokens.lock().await.clone();
        if current.access_token.is_some() && current.access_token != failed_token {
            return Ok(());
        }
        let Some(refresh_token) = current.refresh_token.clone() else {
            self.clear_tokens().await;
            let _ = self.ui.send(UiEvent::WebAuthExpired).await;
            return Err(ApiError::WebAuthExpired);
        };
        let result = self
            .http
            .post(format!("{}/web-auth/refresh", self.base))
            .json(&serde_json::json!({ "refresh_token": refresh_token }))
            .send()
            .await;
        match result {
            Ok(resp) if resp.status().is_success() => {
                match resp.json::<dto::RefreshResponse>().await {
                    Ok(r) => {
                        self.set_tokens(Tokens {
                            access_token: Some(r.access_token),
                            refresh_token: Some(r.refresh_token),
                            access_expires_at: Some(r.access_expires_at),
                            refresh_expires_at: Some(r.refresh_expires_at),
                        })
                        .await;
                        Ok(())
                    }
                    Err(_) => self.fail_refresh().await,
                }
            }
            _ => self.fail_refresh().await,
        }
    }

    async fn fail_refresh(&self) -> Result<(), ApiError> {
        self.clear_tokens().await;
        let _ = self.ui.send(UiEvent::WebAuthExpired).await;
        Err(ApiError::WebAuthExpired)
    }

    /// 主动刷新(过期前 5 分钟缓冲,移植 webAuth store 的 scheduleTokenRefresh)。
    /// 由 AppController 的定时任务调用。
    pub async fn refresh_if_expiring(&self) {
        let tokens = self.tokens.lock().await.clone();
        let (Some(access_exp), Some(refresh_token)) =
            (tokens.access_expires_at, tokens.refresh_token.clone())
        else {
            return;
        };
        let now = chrono::Utc::now().timestamp();
        if access_exp - now > 300 {
            return;
        }
        let resp = self
            .http
            .post(format!("{}/web-auth/refresh", self.base))
            .json(&serde_json::json!({ "refresh_token": refresh_token }))
            .send()
            .await;
        if let Ok(resp) = resp {
            if resp.status().is_success() {
                if let Ok(r) = resp.json::<dto::RefreshResponse>().await {
                    self.set_tokens(Tokens {
                        access_token: Some(r.access_token),
                        refresh_token: Some(r.refresh_token),
                        access_expires_at: Some(r.access_expires_at),
                        refresh_expires_at: Some(r.refresh_expires_at),
                    })
                    .await;
                    return;
                }
            }
        }
        // 刷新失败:清令牌并通知(与 store.refreshTokens 失败路径一致)
        self.clear_tokens().await;
        let _ = self.ui.send(UiEvent::WebAuthExpired).await;
    }
}
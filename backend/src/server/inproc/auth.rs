//! 认证域进程内门面（二维码登录 / Cookie 登录 / 当前用户）。

use axum::extract::{Query, State};
use axum::Json;
use serde_json::Value;

use super::{envelope, InprocError};
use crate::netdisk::StorageQuota;
use crate::server::handlers::auth::{self, QRCodeStatusQuery};
use crate::server::AppState;

pub async fn generate_qrcode(state: AppState) -> Result<Value, InprocError> {
    let Json(resp) = auth::generate_qrcode(State(state))
        .await
        .map_err(InprocError::status)?;
    envelope(resp.code, resp.message, resp.data)
}

pub async fn qrcode_status(
    state: AppState,
    sign: String,
    add: bool,
    first: bool,
) -> Result<Value, InprocError> {
    let Json(resp) = auth::qrcode_status(
        State(state),
        Query(QRCodeStatusQuery { sign, add, first }),
    )
    .await
    .map_err(InprocError::status)?;
    envelope(resp.code, resp.message, resp.data)
}

pub async fn current_user(state: AppState) -> Result<Value, InprocError> {
    let Json(resp) = auth::get_current_user(State(state)).await;
    envelope(resp.code, resp.message, resp.data)
}

pub async fn storage_quota(state: AppState, expected_uid: u64) -> Result<StorageQuota, InprocError> {
    let client = state
        .active_client()
        .await
        .filter(|client| client.uid() == expected_uid)
        .ok_or_else(|| InprocError::new(401, "当前账号的网盘客户端未就绪"))?;
    client.get_storage_quota().await.map_err(|error| {
        tracing::debug!("获取网盘空间失败: {error:#}");
        InprocError::new(502, "无法获取网盘空间")
    })
}

/// Cookie 登录同时返回信封 message（预热提示）。
pub async fn cookie_login(
    state: AppState,
    cookies: String,
) -> Result<(Value, String), InprocError> {
    let req = crate::auth::CookieLoginApiRequest { cookies };
    let Json(resp) = auth::cookie_login(State(state), Json(req)).await;
    if resp.code != 0 {
        return Err(InprocError::new(
            resp.code as i64,
            if resp.message.is_empty() {
                "登录失败".to_string()
            } else {
                resp.message
            },
        ));
    }
    let data = resp.data.ok_or_else(InprocError::missing_data)?;
    Ok((super::to_json(data)?, resp.message))
}

pub async fn logout(state: AppState) -> Result<Value, InprocError> {
    let Json(resp) = auth::logout(State(state)).await;
    envelope(resp.code, resp.message, resp.data)
}

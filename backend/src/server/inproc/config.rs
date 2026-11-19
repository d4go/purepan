//! 配置 / 代理进程内门面。

use axum::extract::State;
use axum::Json;
use serde_json::Value;

use super::{envelope, parse_body, InprocError};
use crate::server::handlers::config;
use crate::server::AppState;

pub async fn get(state: AppState) -> Result<Value, InprocError> {
    let Json(resp) = config::get_config(State(state))
        .await
        .map_err(|e| InprocError::from_api_error(&e))?;
    envelope(resp.code, resp.message, resp.data)
}

pub async fn update(state: AppState, body: Value) -> Result<Value, InprocError> {
    let cfg: crate::config::AppConfig = parse_body(body)?;
    let Json(resp) = config::update_config(State(state), Json(cfg))
        .await
        .map_err(|e| InprocError::from_api_error(&e))?;
    envelope(resp.code, resp.message, resp.data)
}

pub async fn proxy_status(state: AppState) -> Result<Value, InprocError> {
    let Json(resp) = config::get_proxy_status(State(state))
        .await
        .map_err(|e| InprocError::from_api_error(&e))?;
    envelope(resp.code, resp.message, resp.data)
}

pub async fn test_proxy(state: AppState, body: Value) -> Result<Value, InprocError> {
    let _ = state;
    let proxy: crate::common::ProxyConfig = parse_body(body)?;
    let Json(resp) = config::test_proxy_connection(Json(proxy))
        .await
        .map_err(|e| InprocError::from_api_error(&e))?;
    envelope(resp.code, resp.message, resp.data)
}

//! 多账号管理进程内门面。

use axum::extract::{Path, Query, State};
use axum::Json;
use serde_json::Value;

use super::{to_json, InprocError};
use crate::server::handlers::accounts::{
    self, DeleteAccountQuery, SwitchAccountRequest,
};
use crate::server::AppState;

pub async fn list(state: AppState) -> Result<Value, InprocError> {
    let Json(resp) = accounts::list_accounts(State(state))
        .await
        .map_err(|e| InprocError::from_api_error(&e))?;
    to_json(resp)
}

pub async fn switch(state: AppState, uid: u64) -> Result<Value, InprocError> {
    let Json(resp) = accounts::switch_account(State(state), Json(SwitchAccountRequest { uid }))
        .await
        .map_err(|e| InprocError::from_api_error(&e))?;
    to_json(resp)
}

pub async fn delete(state: AppState, uid: u64, force: bool) -> Result<Value, InprocError> {
    let Json(resp) =
        accounts::delete_account(State(state), Path(uid), Query(DeleteAccountQuery { force }))
            .await
            .map_err(|e| InprocError::from_api_error(&e))?;
    to_json(resp)
}

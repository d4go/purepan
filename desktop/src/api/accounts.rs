//! 多账号管理 API(移植 frontend/src/api/accounts.ts)。
//! 注意:这些 handler 返回裸 JSON(不包信封),用 request_flex 兼容两种形态。

use reqwest::Method;
use serde_json::json;

use super::{ApiError, ApiClient};
use crate::api::dto::{DeleteAccountResponse, ListAccountsResponse, SwitchAccountResponse};

impl ApiClient {
    /// GET /accounts/list
    pub async fn list_accounts(&self) -> Result<ListAccountsResponse, ApiError> {
        self.request_flex(Method::GET, "/accounts/list", None).await
    }

    /// POST /accounts/switch
    pub async fn switch_account(&self, uid: u64) -> Result<SwitchAccountResponse, ApiError> {
        self.request_flex(Method::POST, "/accounts/switch", Some(json!({ "uid": uid })))
            .await
    }

    /// DELETE /accounts/:uid?force=true
    pub async fn delete_account(
        &self,
        uid: u64,
        force: bool,
    ) -> Result<DeleteAccountResponse, ApiError> {
        self.request_flex(
            Method::DELETE,
            &format!("/accounts/{uid}?force={force}"),
            None,
        )
        .await
    }
}
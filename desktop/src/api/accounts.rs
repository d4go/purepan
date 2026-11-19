//! 多账号管理 API(进程内直调)。

use purepan_backend::server::inproc;

use super::{ApiClient, ApiError};
use crate::api::dto::{DeleteAccountResponse, ListAccountsResponse, SwitchAccountResponse};

impl ApiClient {
    pub async fn list_accounts(&self) -> Result<ListAccountsResponse, ApiError> {
        self.request(inproc::accounts::list).await
    }

    pub async fn switch_account(&self, uid: u64) -> Result<SwitchAccountResponse, ApiError> {
        self.request(move |s| inproc::accounts::switch(s, uid)).await
    }

    pub async fn delete_account(
        &self,
        uid: u64,
        force: bool,
    ) -> Result<DeleteAccountResponse, ApiError> {
        self.request(move |s| inproc::accounts::delete(s, uid, force))
            .await
    }
}

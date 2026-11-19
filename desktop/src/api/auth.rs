//! 百度账号认证 API(进程内直调)。

use purepan_backend::{netdisk::StorageQuota, server::inproc};

use super::{ApiClient, ApiError};
use crate::api::dto::{QRCode, QRCodeStatus, UserAuth};

impl ApiClient {
    pub async fn generate_qrcode(&self) -> Result<QRCode, ApiError> {
        self.request(inproc::auth::generate_qrcode).await
    }

    pub async fn qrcode_status(
        &self,
        sign: &str,
        add_mode: bool,
        first_check: bool,
    ) -> Result<QRCodeStatus, ApiError> {
        let sign = sign.to_string();
        self.request(move |s| inproc::auth::qrcode_status(s, sign, add_mode, first_check))
            .await
    }

    pub async fn current_user(&self) -> Result<UserAuth, ApiError> {
        self.request(inproc::auth::current_user).await
    }

    pub async fn storage_quota(&self, uid: u64) -> Result<StorageQuota, ApiError> {
        inproc::auth::storage_quota(self.state().await?, uid)
            .await
            .map_err(ApiError::from)
    }

    /// Cookie 登录 → (user, message)
    /// message 含"预热完成"以外内容时 UI 需警告提示。
    pub async fn cookie_login(&self, cookies: &str) -> Result<(UserAuth, String), ApiError> {
        let state = self.state().await?;
        let (value, message) = inproc::auth::cookie_login(state, cookies.to_string())
            .await
            .map_err(ApiError::from)?;
        let user = serde_json::from_value(value)
            .map_err(|e| ApiError::Decode(e.to_string()))?;
        Ok((user, message))
    }

    pub async fn logout(&self) -> Result<(), ApiError> {
        let _: serde_json::Value = self.request(inproc::auth::logout).await?;
        Ok(())
    }
}

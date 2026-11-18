//! 百度账号认证 API(移植 frontend/src/api/auth.ts)。

use reqwest::Method;
use serde_json::json;

use super::{ApiError, ApiClient};
use crate::api::dto::{QRCode, QRCodeStatus, UserAuth};

impl ApiClient {
    /// POST /auth/qrcode/generate
    pub async fn generate_qrcode(&self) -> Result<QRCode, ApiError> {
        self.request(Method::POST, "/auth/qrcode/generate", Some(json!({})))
            .await
    }

    /// GET /auth/qrcode/status?sign=&add=&first=
    pub async fn qrcode_status(
        &self,
        sign: &str,
        add_mode: bool,
        first_check: bool,
    ) -> Result<QRCodeStatus, ApiError> {
        let mut path = format!("/auth/qrcode/status?sign={}", urlencoded(sign));
        if add_mode {
            path.push_str("&add=true");
        }
        if first_check {
            path.push_str("&first=true");
        }
        self.request(Method::GET, &path, None).await
    }

    /// GET /auth/user
    pub async fn current_user(&self) -> Result<UserAuth, ApiError> {
        self.request(Method::GET, "/auth/user", None).await
    }

    /// POST /auth/cookie/login → (user, message)
    /// message 含"预热完成"以外内容时 UI 需警告提示。
    pub async fn cookie_login(&self, cookies: &str) -> Result<(UserAuth, String), ApiError> {
        self.request_with_message(
            Method::POST,
            "/auth/cookie/login",
            Some(json!({ "cookies": cookies })),
        )
        .await
    }

    /// POST /auth/logout
    pub async fn logout(&self) -> Result<(), ApiError> {
        self.request::<serde_json::Value>(Method::POST, "/auth/logout", Some(json!({})))
            .await?;
        Ok(())
    }
}

fn urlencoded(s: &str) -> String {
    // sign 为 URL 安全字符,但稳妥起见做百分号编码
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}
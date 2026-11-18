//! Web 访问认证 API(移植 frontend/src/api/webAuth.ts)。
//! login/refresh 是认证端点本身,不走 419 重试(与 TS 拦截器排除逻辑一致)。

use reqwest::Method;
use serde_json::json;

use super::{ApiError, ApiClient, Tokens};
use crate::api::dto::{
    AuthConfigResponse, AuthStatusResponse, RegenerateCodesResponse, TotpSetupResponse,
    WebLoginRequest, WebLoginResponse,
};

impl ApiClient {
    /// POST /web-auth/login(裸 JSON,无信封)
    pub async fn web_login(&self, req: WebLoginRequest) -> Result<WebLoginResponse, ApiError> {
        let resp = self
            .http
            .post(format!("{}/web-auth/login", self.base))
            .json(&req)
            .send()
            .await
            .map_err(|e| ApiError::Network(e.to_string()))?;
        let status = resp.status();
        let text = resp
            .text()
            .await
            .map_err(|e| ApiError::Network(e.to_string()))?;
        if !status.is_success() {
            // 错误响应可能是 {code,error,message,details:{lockout_remaining}}
            let v: serde_json::Value = serde_json::from_str(&text).unwrap_or_default();
            let message = v
                .get("message")
                .or_else(|| v.get("error"))
                .and_then(|m| m.as_str())
                .unwrap_or("登录失败")
                .to_string();
            let lockout = v
                .get("details")
                .and_then(|d| d.get("lockout_remaining"))
                .and_then(|l| l.as_i64());
            return Ok(WebLoginResponse {
                status: "error".into(),
                access_token: None,
                refresh_token: None,
                access_expires_at: None,
                refresh_expires_at: None,
                pending_token: None,
                error: Some(message),
                lockout_remaining: lockout,
            });
        }
        serde_json::from_str(&text).map_err(|e| ApiError::Decode(e.to_string()))
    }

    /// POST /web-auth/logout
    pub async fn web_logout(&self) -> Result<(), ApiError> {
        let _ = self
            .send_authorized(Method::POST, "/web-auth/logout", Some(json!({})))
            .await;
        self.clear_tokens().await;
        Ok(())
    }

    /// GET /web-auth/status
    pub async fn web_status(&self) -> Result<AuthStatusResponse, ApiError> {
        self.request_raw(Method::GET, "/web-auth/status", None).await
    }

    /// GET /web-auth/config
    pub async fn web_config(&self) -> Result<AuthConfigResponse, ApiError> {
        self.request_raw(Method::GET, "/web-auth/config", None).await
    }

    /// PUT /web-auth/config
    pub async fn web_update_config(
        &self,
        enabled: Option<bool>,
        mode: Option<crate::api::dto::AuthMode>,
    ) -> Result<(), ApiError> {
        let mut body = serde_json::Map::new();
        if let Some(e) = enabled {
            body.insert("enabled".into(), json!(e));
        }
        if let Some(m) = mode {
            body.insert("mode".into(), json!(m));
        }
        self.send_authorized(
            Method::PUT,
            "/web-auth/config",
            Some(serde_json::Value::Object(body)),
        )
        .await?;
        Ok(())
    }

    /// POST /web-auth/password/set
    pub async fn web_set_password(
        &self,
        password: &str,
        current_password: Option<&str>,
    ) -> Result<(), ApiError> {
        let mut body = json!({ "password": password });
        if let Some(cur) = current_password {
            body["current_password"] = json!(cur);
        }
        self.send_authorized(Method::POST, "/web-auth/password/set", Some(body))
            .await?;
        Ok(())
    }

    /// POST /web-auth/totp/setup
    pub async fn web_totp_setup(&self) -> Result<TotpSetupResponse, ApiError> {
        self.request_raw(Method::POST, "/web-auth/totp/setup", Some(json!({})))
            .await
    }

    /// POST /web-auth/totp/verify
    pub async fn web_totp_verify(&self, code: &str, secret: Option<&str>) -> Result<(), ApiError> {
        let mut body = json!({ "code": code });
        if let Some(s) = secret {
            body["secret"] = json!(s);
        }
        self.send_authorized(Method::POST, "/web-auth/totp/verify", Some(body))
            .await?;
        Ok(())
    }

    /// POST /web-auth/totp/disable
    pub async fn web_totp_disable(
        &self,
        code: Option<&str>,
        recovery_code: Option<&str>,
    ) -> Result<(), ApiError> {
        let mut body = serde_json::Map::new();
        if let Some(c) = code {
            body.insert("code".into(), json!(c));
        }
        if let Some(r) = recovery_code {
            body.insert("recovery_code".into(), json!(r));
        }
        self.send_authorized(
            Method::POST,
            "/web-auth/totp/disable",
            Some(serde_json::Value::Object(body)),
        )
        .await?;
        Ok(())
    }

    /// POST /web-auth/recovery-codes/regenerate
    pub async fn web_regenerate_recovery_codes(
        &self,
        totp_code: &str,
    ) -> Result<RegenerateCodesResponse, ApiError> {
        self.request_raw(
            Method::POST,
            "/web-auth/recovery-codes/regenerate",
            Some(json!({ "totp_code": totp_code })),
        )
        .await
    }

    /// 登录成功后保存令牌(供视图调用)。
    pub async fn save_login_tokens(&self, resp: &WebLoginResponse) {
        if resp.access_token.is_some() {
            self.set_tokens(Tokens {
                access_token: resp.access_token.clone(),
                refresh_token: resp.refresh_token.clone(),
                access_expires_at: resp.access_expires_at,
                refresh_expires_at: resp.refresh_expires_at,
            })
            .await;
        }
    }
}
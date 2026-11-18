//! Web 访问认证 UI 状态(移植 frontend/src/stores/webAuth.ts)。
//! 令牌本体由 ApiClient 持有并持久化;这里保存登录流程的界面状态。

use gpui_kit::Global;

use crate::api::dto::{AuthMode, AuthStatusResponse};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LoginStep {
    #[default]
    Password,
    Totp,
    Complete,
}

#[derive(Default)]
pub struct WebAuthState {
    pub is_authenticated: bool,
    pub auth_config: Option<AuthStatusResponse>,
    pub login_step: LoginStep,
    pub pending_login_token: Option<String>,
    pub error: Option<String>,
    pub lockout_remaining: Option<i64>,
}

impl Global for WebAuthState {}

impl WebAuthState {
    pub fn is_auth_enabled(&self) -> bool {
        self.auth_config.as_ref().map(|c| c.enabled).unwrap_or(false)
    }

    pub fn auth_mode(&self) -> AuthMode {
        self.auth_config
            .as_ref()
            .map(|c| c.mode)
            .unwrap_or(AuthMode::None)
    }

    pub fn requires_password(&self) -> bool {
        matches!(self.auth_mode(), AuthMode::Password | AuthMode::PasswordTotp)
    }

    pub fn requires_totp(&self) -> bool {
        matches!(self.auth_mode(), AuthMode::Totp | AuthMode::PasswordTotp)
    }

    pub fn reset_login_flow(&mut self) {
        self.login_step = LoginStep::Password;
        self.pending_login_token = None;
        self.error = None;
        self.lockout_remaining = None;
    }
}
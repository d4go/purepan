//! 多账号认证状态(移植 frontend/src/stores/auth.ts 的状态与计算属性)。
//! API 动作在视图/控制器里经 bridge::call 调用,这里只保存状态与纯函数应用器。

use gpui_kit::Global;
use purepan_backend::auth::types::{AccountSummary, UserAuth};
use purepan_backend::server::events::AccountEvent;
use purepan_backend::server::websocket::WsServerMessage;

#[derive(Default)]
pub struct AuthStore {
    /// 兼容字段:旧路径登录态(QR/cookie 登录成功时填充)
    pub user: Option<UserAuth>,
    pub accounts: Vec<AccountSummary>,
    pub active_uid: Option<u64>,
    pub readonly_mode: bool,
    pub accounts_loaded: bool,
}

impl Global for AuthStore {}

impl AuthStore {
    pub fn is_logged_in(&self) -> bool {
        self.user.is_some() || self.active_uid.is_some()
    }

    pub fn active_account(&self) -> Option<&AccountSummary> {
        let uid = self.active_uid?;
        self.accounts.iter().find(|a| a.uid == uid)
    }

    pub fn username(&self) -> String {
        if let Some(acc) = self.active_account() {
            return acc
                .nickname
                .clone()
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| acc.username.clone());
        }
        self.user
            .as_ref()
            .map(|u| {
                u.nickname
                    .clone()
                    .filter(|s| !s.is_empty())
                    .unwrap_or_else(|| u.username.clone())
            })
            .unwrap_or_default()
    }

    pub fn avatar(&self) -> String {
        if let Some(acc) = self.active_account() {
            return acc.avatar_url.clone().unwrap_or_default();
        }
        self.user
            .as_ref()
            .and_then(|u| u.avatar_url.clone())
            .unwrap_or_default()
    }

    pub fn has_multiple_accounts(&self) -> bool {
        self.accounts.len() > 1
    }

    /// 当前用户完整信息:优先 activeAccount,再用旧 user 补充扩展字段。
    pub fn current_user(&self) -> Option<UserAuth> {
        if let Some(acc) = self.active_account() {
            let mut u = UserAuth::new(acc.uid, acc.username.clone(), String::new());
            u.nickname = acc.nickname.clone();
            u.avatar_url = acc.avatar_url.clone();
            u.vip_type = acc.vip_type;
            if let Some(old) = &self.user {
                u.login_time = old.login_time;
                u.total_space = old.total_space;
                u.used_space = old.used_space;
            }
            return Some(u);
        }
        self.user.clone()
    }

    /// 应用 WS 快照中的多账号字段(移植 applySnapshot)。
    /// accounts/readonly_mode 后端总是发送(default),直接覆盖;
    /// active_uid 缺失(skip_serializing_if)时保持当前值。
    pub fn apply_snapshot(&mut self, msg: &WsServerMessage) {
        if let WsServerMessage::Snapshot {
            accounts,
            active_uid,
            readonly_mode,
            ..
        } = msg
        {
            self.accounts = accounts.clone();
            self.accounts_loaded = true;
            if let Some(uid) = active_uid {
                self.active_uid = Some(*uid);
            }
            self.readonly_mode = *readonly_mode;
        }
    }

    /// 应用账号事件(移植 applyAccountEvent)。
    pub fn apply_account_event(&mut self, event: &AccountEvent) {
        match event {
            AccountEvent::Switched { new_active_uid } => {
                self.active_uid = *new_active_uid;
                for a in &mut self.accounts {
                    a.is_active = Some(a.uid) == *new_active_uid;
                }
            }
            AccountEvent::ListChanged {
                accounts,
                active_uid,
            } => {
                self.accounts = accounts.clone();
                self.active_uid = *active_uid;
                self.accounts_loaded = true;
            }
        }
    }

    pub fn clear(&mut self) {
        self.user = None;
        self.accounts.clear();
        self.active_uid = None;
        self.readonly_mode = false;
        self.accounts_loaded = false;
    }
}
pub mod auth;
pub mod health;
pub mod task_cache;

pub use crate::backend_embed::Backend;
pub use auth::AuthStore;
pub use health::BackendHealth;
pub use task_cache::TaskCache;

use gpui_kit::Global;

/// 外观主题偏好。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ThemePref {
    Light,
    Dark,
    #[default]
    System,
}

/// 桌面 UI 偏好(主题等),运行期由设置页与窗口观察者读取。
#[derive(Debug, Clone, Copy, Default)]
pub struct UiPrefs {
    pub theme: ThemePref,
}

impl Global for UiPrefs {}
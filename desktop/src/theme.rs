//! 外观主题:浅色 / 深色 / 跟随系统。
//! 偏好持久化在 `[ui] theme` 配置中,启动时由 probe_boot 读取并应用;
//! 运行中可在设置页切换,系统外观变化时由窗口观察者自动同步(System 模式)。

use gpui_kit::App;
use gpui_kit::component::{Theme, ThemeMode};
use gpui_kit::WindowAppearance;

use purepan_backend::config::ThemeSetting;

use crate::state::{ThemePref, UiPrefs};

pub fn init(cx: &mut App) {
    // 兜底:首帧先跟随系统,probe_boot 读到配置后会覆盖
    Theme::sync_system_appearance(None, cx);
}

fn pref_from_setting(setting: ThemeSetting) -> ThemePref {
    match setting {
        ThemeSetting::Light => ThemePref::Light,
        ThemeSetting::Dark => ThemePref::Dark,
        ThemeSetting::System => ThemePref::System,
    }
}

/// 应用主题偏好:同时切换组件库主题与原生窗口外观(标题栏/边框)。
pub fn apply_pref(pref: ThemePref, cx: &mut App) {
    cx.set_global(UiPrefs { theme: pref });
    match pref {
        ThemePref::Light => {
            cx.set_window_appearance(Some(WindowAppearance::Light));
            Theme::change(ThemeMode::Light, None, cx);
        }
        ThemePref::Dark => {
            cx.set_window_appearance(Some(WindowAppearance::Dark));
            Theme::change(ThemeMode::Dark, None, cx);
        }
        ThemePref::System => {
            cx.set_window_appearance(None);
            Theme::sync_system_appearance(None, cx);
        }
    }
}

/// 从后端配置应用主题(启动探测时调用)。
pub fn apply_setting(setting: ThemeSetting, cx: &mut App) {
    apply_pref(pref_from_setting(setting), cx);
}

/// System 模式下,窗口外观变化时重新同步主题。
pub fn sync_if_system(window: &mut gpui_kit::Window, cx: &mut App) {
    if cx.global::<UiPrefs>().theme == ThemePref::System {
        Theme::sync_system_appearance(Some(window), cx);
    }
}
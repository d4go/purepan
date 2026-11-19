use gpui_kit::{App, Window};

/// 读取 `config/app.toml` 的 `[ui] close_behavior`(每次关闭都重读,设置改动即时生效)。
/// 移植自 src-tauri/src/main.rs 的 read_close_behavior。
pub fn read_close_behavior() -> String {
    if let Ok(content) = std::fs::read_to_string("config/app.toml") {
        if let Ok(value) = content.parse::<toml::Value>() {
            if let Some(behavior) = value
                .get("ui")
                .and_then(|ui| ui.get("close_behavior"))
                .and_then(|v| v.as_str())
            {
                return behavior.to_string();
            }
        }
    }
    "close".to_string()
}

/// 统一的关闭入口:TitleBar 关闭按钮(Linux)与系统关闭事件都走这里。
/// minimize_to_tray 时最小化代替关闭;选择退出时必须结束进程,
/// 否则 macOS 关闭窗口后会驻留 Dock 且无法再打开界面。
pub fn request_close(window: &mut Window, cx: &mut App) {
    if read_close_behavior() == "minimize_to_tray" {
        window.minimize_window();
    } else {
        cx.quit();
    }
}

/// 系统关闭请求(Windows 标题栏/Alt+F4、macOS 红绿灯)的守卫。
/// 返回 false 取消关闭。
pub fn should_close(window: &mut Window, cx: &mut App) -> bool {
    if read_close_behavior() == "minimize_to_tray" {
        window.minimize_window();
        false
    } else {
        cx.quit();
        true
    }
}

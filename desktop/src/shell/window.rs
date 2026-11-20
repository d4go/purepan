use std::collections::HashMap;

use gpui_kit::component::WindowExt as _;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::dialog::Dialog;
use gpui_kit::component::h_flex;
use gpui_kit::{App, ParentElement as _, Styled as _, Window, px};
use serde_json::Value;

use crate::components::task_ui;
use crate::state::TaskCache;

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
        quit_with_confirm(window, cx);
    }
}

/// 系统关闭请求(Windows 标题栏/Alt+F4、macOS 红绿灯)的守卫。
/// 恒返回 false:退出统一由 quit_with_confirm 接管(无任务直接退,有任务先弹确认)。
pub fn should_close(window: &mut Window, cx: &mut App) -> bool {
    if read_close_behavior() == "minimize_to_tray" {
        window.minimize_window();
    } else {
        quit_with_confirm(window, cx);
    }
    false
}

/// 有活跃任务时先弹确认框,否则直接退出。托盘"退出"等显式退出入口同样走这里。
pub fn quit_with_confirm(window: &mut Window, cx: &mut App) {
    let (downloads, uploads, transfers, folders) = active_task_counts(cx);
    let total = downloads + uploads + transfers + folders;
    if total == 0 {
        cx.quit();
        return;
    }
    if window.has_active_dialog(cx) {
        return;
    }
    let detail = [
        (downloads, "下载"),
        (uploads, "上传"),
        (transfers, "转存"),
        (folders, "备份"),
    ]
    .iter()
    .filter(|(n, _)| *n > 0)
    .map(|(n, name)| format!("{name} {n}"))
    .collect::<Vec<_>>()
    .join(" · ");
    window.activate_window();
    window.open_dialog(cx, move |dialog: Dialog, _, _| {
        dialog
            .title("确认退出")
            .width(px(460.))
            .child(format!(
                "当前有 {total} 个任务进行中（{detail}），退出后将中断这些任务。"
            ))
            .footer(
                h_flex()
                    .gap_2()
                    .justify_end()
                    .child(
                        Button::new("quit-cancel")
                            .label("取消")
                            .on_click(|_, w, cx| w.close_dialog(cx)),
                    )
                    .child(
                        Button::new("quit-ok")
                            .danger()
                            .label("退出")
                            .on_click(|_, _, cx| cx.quit()),
                    ),
            )
    });
}

fn active_task_counts(cx: &App) -> (usize, usize, usize, usize) {
    fn count(map: &HashMap<String, Value>) -> usize {
        map.values()
            .filter(|v| {
                v.get("status")
                    .and_then(|s| s.as_str())
                    .map(task_ui::is_active_status)
                    .unwrap_or(false)
            })
            .count()
    }
    let cache = cx.global::<TaskCache>();
    (
        count(&cache.downloads),
        count(&cache.uploads),
        count(&cache.transfers),
        count(&cache.folders),
    )
}
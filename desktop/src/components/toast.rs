//! 统一 Notification 提示(对应 Vue 的 ElMessage)。

use gpui_kit::component::WindowExt as _;
use gpui_kit::component::notification::Notification;
use gpui_kit::{App, SharedString, Window};

use crate::bridge::ToastKind;

pub fn show(
    window: &mut Window,
    cx: &mut App,
    kind: ToastKind,
    message: impl Into<SharedString>,
) {
    let note = match kind {
        ToastKind::Info => Notification::info(message),
        ToastKind::Success => Notification::success(message),
        ToastKind::Warning => Notification::warning(message),
        ToastKind::Error => Notification::error(message),
    };
    window.push_notification(note, cx);
}
use gpui_kit::component::{TitleBar, h_flex};
use gpui_kit::{ParentElement as _, Styled as _};

use crate::shell;

/// 自定义标题栏:左侧应用名,右侧窗口控制按钮(组件库自带)。
/// 关闭行为统一走 shell::window(关闭到托盘逻辑)。
pub fn render() -> TitleBar {
    TitleBar::new()
        .child(h_flex().items_center().child("PurePan"))
        .on_close_window(|_, window, _| shell::window::request_close(window))
}
use gpui_kit::component::TitleBar;

use crate::shell;

/// 自定义标题栏:仅右侧窗口控制按钮(组件库自带),不显示 logo 与应用名。
/// 关闭行为统一走 shell::window(关闭到托盘逻辑)。
pub fn render() -> TitleBar {
    TitleBar::new().on_close_window(|_, window, _| shell::window::request_close(window))
}

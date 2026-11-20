use gpui_kit::component::TitleBar;
use gpui_kit::{Hsla, Styled as _, px};

use crate::shell;

/// 自定义标题栏:仅右侧窗口控制按钮(组件库自带),不显示 logo 与应用名。
/// 背景由调用方传入:主窗=chrome 色,登录/启动窗=canvas(与页面背景一致)。
/// 高度从组件默认 34px 压到 28px(macOS 原生规格),降低顶部视觉厚度。
/// 关闭行为统一走 shell::window(关闭到托盘逻辑)。
pub fn render(bg: Hsla) -> TitleBar {
    TitleBar::new()
        .h(px(28.))
        .bg(bg)
        .border_b_0()
        .on_close_window(|_, window, cx| shell::window::request_close(window, cx))
}

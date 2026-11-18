use gpui_kit::component::{TitleBar, h_flex};
use gpui_kit::{ParentElement as _, Styled as _, img, px};

use crate::shell;
use crate::util;

/// 自定义标题栏:左侧 logo + 应用名,右侧窗口控制按钮(组件库自带)。
/// 关闭行为统一走 shell::window(关闭到托盘逻辑)。
pub fn render() -> TitleBar {
    TitleBar::new()
        .child(
            h_flex()
                .items_center()
                .gap_1p5()
                .child(img(util::app_logo()).size(px(16.)).rounded(px(3.)))
                .child("PurePan"),
        )
        .on_close_window(|_, window, _| shell::window::request_close(window))
}
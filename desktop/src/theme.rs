use gpui_kit::App;
use gpui_kit::component::Theme;

pub fn init(cx: &mut App) {
    // 亮暗跟随系统(后续 appearance 变化由组件库监听)
    Theme::sync_system_appearance(None, cx);
}
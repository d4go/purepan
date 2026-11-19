//! 尚未迁移页面的占位视图(P2 起逐页替换)。

use gpui_kit::component::{ActiveTheme as _, Icon, v_flex};
use gpui_kit::{Context, IntoElement, ParentElement as _, Render, Styled as _, Window, div, px};

use crate::components::app_sidebar::Nav;

pub struct Placeholder {
    nav: Nav,
}

impl Placeholder {
    pub fn new(nav: Nav) -> Self {
        Self { nav }
    }
}

impl Render for Placeholder {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        div()
            .flex_1()
            .flex()
            .items_center()
            .justify_center()
            .child(
                v_flex()
                    .items_center()
                    .gap_2()
                    .child(
                        Icon::new(self.nav.icon())
                            .size(px(48.))
                            .text_color(theme.muted_foreground),
                    )
                    .child(format!("「{}」界面迁移中", self.nav.label())),
            )
    }
}
//! 网盘与本地文件浏览页共用的顶栏、列表表头和加载状态。

use gpui_kit::component::scroll::{Scrollable, ScrollableElement as _};
use gpui_kit::component::spinner::Spinner;
use gpui_kit::component::{Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AnyElement, Div, InteractiveElement as _, IntoElement, ParentElement as _, Stateful,
    Styled as _, div, px,
};

use crate::design::Palette;

pub const FOLDER_ICON_COLOR: u32 = 0xF59E0B;

pub fn header(
    breadcrumb: AnyElement,
    count: Option<usize>,
    is_search: bool,
    tools: impl IntoElement,
    p: &Palette,
) -> AnyElement {
    h_flex()
        .h(px(68.))
        .px(px(32.))
        .gap(px(16.))
        .items_center()
        .flex_shrink_0()
        .child(
            h_flex()
                .gap(px(12.))
                .items_center()
                .flex_1()
                .min_w_0()
                .child(div().min_w_0().overflow_hidden().child(breadcrumb))
                .when_some(count, |row, count| {
                    row.child(
                        div()
                            .flex_shrink_0()
                            .text_size(px(12.))
                            .text_color(p.faint)
                            .child(if is_search {
                                format!("搜索结果 · {count} 项")
                            } else {
                                format!("{count} 项")
                            }),
                    )
                }),
        )
        .child(tools)
        .into_any_element()
}

pub fn scroll(id: &'static str) -> Scrollable<Stateful<Div>> {
    div()
        .id(id)
        .flex_1()
        .min_h_0()
        .overflow_y_scrollbar()
        .px(px(32.))
        .py(px(24.))
}

pub fn list_header(p: &Palette) -> AnyElement {
    h_flex()
        .h(px(34.))
        .px(px(12.))
        .gap(px(10.))
        .items_center()
        .text_size(px(12.))
        .text_color(p.faint)
        .child(div().w(px(28.)))
        .child(div().w(px(22.)))
        .child(div().flex_1().child("名称"))
        .child(div().w(px(120.)).text_center().child("大小"))
        .child(div().w(px(170.)).pl(px(28.)).child("修改时间"))
        .child(div().w(px(36.)))
        .into_any_element()
}

pub fn loading_state(p: &Palette, label: &'static str) -> AnyElement {
    v_flex()
        .flex_1()
        .items_center()
        .justify_center()
        .gap(px(10.))
        .text_size(px(14.))
        .text_color(p.muted)
        .child(Spinner::new().with_size(px(24.)).color(p.primary))
        .child(label)
        .into_any_element()
}

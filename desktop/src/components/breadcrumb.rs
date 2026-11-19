//! 面包屑导航(根目录 + 可点击的路径分段)。

use gpui_kit::component::breadcrumb::{Breadcrumb, BreadcrumbItem};
use gpui_kit::{App, ClickEvent, Window};

/// on_navigate 为每个分段生成点击回调(目标路径)。
pub fn render<G>(dir: &str, on_navigate: impl Fn(String) -> G) -> Breadcrumb
where
    G: Fn(&ClickEvent, &mut Window, &mut App) + 'static,
{
    let mut bc = Breadcrumb::new().child(
        BreadcrumbItem::new("根目录").on_click(on_navigate("/".to_string())),
    );
    if dir != "/" {
        let mut acc = String::new();
        for part in dir.split('/').filter(|p| !p.is_empty()) {
            acc.push('/');
            acc.push_str(part);
            bc = bc.child(
                BreadcrumbItem::new(part.to_string()).on_click(on_navigate(acc.clone())),
            );
        }
    }
    bc
}
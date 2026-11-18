//! 本地文件页(移植 LocalFilesView.vue):浏览下载目录,行点击进目录,批量删除(不可逆)。

use std::collections::HashSet;
use std::sync::Arc;

use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::component::dialog::Dialog;
use gpui_kit::component::menu::{PopupMenu, PopupMenuItem};
use gpui_kit::component::table::{Column, DataTable, TableDelegate, TableState};
use gpui_kit::component::{ActiveTheme as _, Icon, Sizable as _, WindowExt as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AnyWindowHandle, App, AppContext as _, Context, Div, Entity, InteractiveElement as _,
    IntoElement, ParentElement as _, Render, Stateful, StatefulInteractiveElement as _, Styled as _,
    Subscription, WeakEntity, Window, div, px,
};

use crate::api::filesystem::{EntryType, FileEntry};
use crate::api::{self, ApiClient};
use crate::bridge::{EventHub, ToastKind, UiEvent};
use crate::components::{breadcrumb, toast};
use crate::state::Backend;
use crate::util;

pub struct LocalDelegate {
    pub entries: Vec<FileEntry>,
    pub selected: HashSet<String>,
    pub current_dir: String,
    parent: Option<String>,
    has_more: bool,
    page: usize,
    loading: bool,
    loading_more: bool,
    rt: tokio::runtime::Handle,
    api: Arc<ApiClient>,
    view: WeakEntity<LocalFilesView>,
}

impl LocalDelegate {
    fn new(rt: tokio::runtime::Handle, api: Arc<ApiClient>, view: WeakEntity<LocalFilesView>) -> Self {
        Self {
            entries: Vec::new(),
            selected: HashSet::new(),
            current_dir: String::new(),
            parent: None,
            has_more: false,
            page: 0,
            loading: false,
            loading_more: false,
            rt,
            api,
            view,
        }
    }

    pub fn load(&mut self, path: &str, append: bool, cx: &mut Context<TableState<Self>>) {
        if append && (self.loading || self.loading_more || !self.has_more) {
            return;
        }
        if append { self.loading_more = true; } else { self.loading = true; self.page = 0; }
        let path = path.to_string();
        let page = if append { self.page + 1 } else { 0 };
        let (rt, api) = (self.rt.clone(), self.api.clone());
        cx.notify();
        cx.spawn(async move |this, cx| {
            let (api2, path2) = (api.clone(), path.clone());
            let result = api::call_api(&rt, move || async move {
                api2.list_local_files(&path2, page, 100).await
            }).await;
            let _ = this.update(cx, |state, cx| {
                let d = state.delegate_mut();
                d.loading = false;
                d.loading_more = false;
                match result {
                    Ok(data) => {
                        if append { d.entries.extend(data.entries); } else { d.entries = data.entries; d.selected.clear(); }
                        d.current_dir = data.current_path;
                        d.parent = data.parent_path;
                        d.has_more = data.has_more;
                        d.page = data.page;
                    }
                    Err(e) => {
                        let msg = match &e {
                            api::ApiError::Business { code, message } => crate::api::filesystem::friendly_error(*code, message),
                            _ => e.message(),
                        };
                        let _ = d.view.update(cx, |v, cx| v.toast(cx, ToastKind::Error, msg));
                    }
                }
                cx.notify();
                let _ = d.view.update(cx, |_, cx| cx.notify());
            });
        }).detach();
    }

    pub fn selected_entries(&self) -> Vec<FileEntry> {
        self.entries.iter().filter(|e| self.selected.contains(&e.path)).cloned().collect()
    }
}

impl TableDelegate for LocalDelegate {
    fn columns_count(&self, _: &App) -> usize { 4 }
    fn rows_count(&self, _: &App) -> usize { self.entries.len() }
    fn column(&self, col_ix: usize, _: &App) -> Column {
        match col_ix {
            0 => Column::new("sel", "").width(px(48.)),
            1 => Column::new("name", "文件名").min_width(px(320.)),
            2 => Column::new("size", "大小").width(px(100.)),
            3 => Column::new("mtime", "修改时间").width(px(180.)),
            _ => Column::new("x", ""),
        }
    }
    fn render_tr(&mut self, row_ix: usize, _: &mut Window, _: &mut Context<TableState<Self>>) -> Stateful<Div> {
        let view = self.view.clone();
        let target = self.entries.get(row_ix).map(|e| (e.entry_type == EntryType::Directory, e.path.clone()));
        div().id(("local-row", row_ix)).on_click(move |_, _, cx| {
            if let Some((true, path)) = &target {
                let path = path.clone();
                let _ = view.update(cx, |v, cx| v.navigate(path, cx));
            }
        })
    }
    fn context_menu(&mut self, row_ix: usize, menu: PopupMenu, _: &mut Window, _: &mut Context<TableState<Self>>) -> PopupMenu {
        let Some(e) = self.entries.get(row_ix).cloned() else { return menu };
        let view = self.view.clone();
        menu.item(
            PopupMenuItem::new("删除").icon(IconName::Trash).on_click(move |_, _, cx| {
                let e = e.clone();
                let _ = view.update(cx, |v, cx| v.open_delete(vec![e], cx));
            }),
        )
    }
    fn render_td(&mut self, row_ix: usize, col_ix: usize, _: &mut Window, cx: &mut Context<TableState<Self>>) -> impl IntoElement {
        let Some(e) = self.entries.get(row_ix).cloned() else { return div().into_any_element() };
        let theme = cx.theme().clone();
        match col_ix {
            0 => {
                let path = e.path.clone();
                let checked = self.selected.contains(&path);
                let view = self.view.clone();
                div().flex().items_center().justify_center()
                    .on_mouse_down(gpui_kit::MouseButton::Left, |_, w, cx| { w.prevent_default(); cx.stop_propagation(); })
                    .child(Checkbox::new(("lsel", row_ix)).checked(checked).on_change(move |c, _, cx| {
                        let c = *c; let p = path.clone();
                        let _ = view.update(cx, |v, cx| v.toggle(p, c, cx));
                    }))
                    .into_any_element()
            }
            1 => {
                let is_dir = e.entry_type == EntryType::Directory;
                h_flex().gap_2().items_center()
                    .child(Icon::new(if is_dir { IconName::Folder } else { IconName::FileText }).size(px(20.)).text_color(if is_dir { theme.primary } else { theme.muted_foreground }))
                    .child(div().truncate().child(e.name.clone()))
                    .into_any_element()
            }
            2 => div().w_full().text_right().child(if e.entry_type == EntryType::Directory { "-".to_string() } else { e.size.map(util::format_size).unwrap_or_else(|| "-".into()) }).into_any_element(),
            3 => div().child(e.updated_at.clone()).into_any_element(),
            _ => div().into_any_element(),
        }
    }
    fn has_more(&self, _: &App) -> bool { self.has_more }
    fn load_more_threshold(&self) -> usize { 10 }
    fn load_more(&mut self, _: &mut Window, cx: &mut Context<TableState<Self>>) {
        let dir = self.current_dir.clone();
        self.load(&dir, true, cx);
    }
    fn loading(&self, _: &App) -> bool { self.loading }
    fn render_empty(&mut self, _: &mut Window, cx: &mut Context<TableState<Self>>) -> impl IntoElement {
        h_flex().size_full().justify_center().items_center().text_color(cx.theme().muted_foreground).child("当前目录为空").into_any_element()
    }
}

pub struct LocalFilesView {
    table: Entity<TableState<LocalDelegate>>,
    delete_targets: Vec<FileEntry>,
    window_handle: AnyWindowHandle,
    _subscriptions: Vec<Subscription>,
}

impl LocalFilesView {
    pub fn new(hub: Entity<EventHub>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let b = cx.global::<Backend>();
        let (rt, api) = (b.rt.clone(), b.api.clone());
        let view = cx.entity().downgrade();
        let table = cx.new(|cx| TableState::new(LocalDelegate::new(rt, api, view), window, cx));
        let sub = cx.subscribe(&hub, |this, _, ev, cx| {
            if matches!(ev, UiEvent::AccountChanged { .. }) {
                this.navigate(String::new(), cx);
            }
        });
        let mut v = Self { table, delete_targets: Vec::new(), window_handle: window.window_handle(), _subscriptions: vec![sub] };
        v.navigate(String::new(), cx);
        v
    }

    pub fn navigate(&mut self, path: String, cx: &mut Context<Self>) {
        self.table.update(cx, |state, cx| state.delegate_mut().load(&path, false, cx));
        cx.notify();
    }

    fn toggle(&mut self, path: String, checked: bool, cx: &mut Context<Self>) {
        self.table.update(cx, |state, cx| {
            let d = state.delegate_mut();
            if checked { d.selected.insert(path); } else { d.selected.remove(&path); }
            cx.notify();
        });
        cx.notify();
    }

    fn toast(&self, cx: &mut Context<Self>, kind: ToastKind, msg: impl Into<String>) {
        let msg = msg.into();
        let wh = self.window_handle;
        let _ = wh.update(cx, |_, w, cx| toast::show(w, cx, kind, msg.clone()));
    }

    fn open_delete(&mut self, files: Vec<FileEntry>, cx: &mut Context<Self>) {
        self.delete_targets = files;
        let count = self.delete_targets.len();
        let view = cx.entity().clone();
        let wh = self.window_handle;
        let _ = wh.update(cx, move |_, window, cx| {
            let view2 = view.clone();
            window.open_dialog(cx, move |dialog: Dialog, _, _| {
                let view3 = view2.clone();
                dialog.title("删除本地文件").width(px(460.))
                    .child(format!("确定删除选中的 {count} 项吗?本地文件删除后不可恢复!"))
                    .footer(h_flex().gap_2().justify_end()
                        .child(Button::new("ld-cancel").label("取消").on_click(|_, w, cx| w.close_dialog(cx)))
                        .child(Button::new("ld-ok").danger().label("删除").on_click(move |_, _, cx| {
                            view3.update(cx, |v, cx| v.confirm_delete(cx));
                        })))
                    .on_ok({ let v = view2.clone(); move |_, _, cx| { v.update(cx, |v, cx| v.confirm_delete(cx)); true } })
            });
        });
    }

    fn confirm_delete(&mut self, cx: &mut Context<Self>) {
        let paths: Vec<String> = self.delete_targets.iter().map(|e| e.path.clone()).collect();
        if paths.is_empty() { return; }
        let b = cx.global::<Backend>();
        let (rt, api) = (b.rt.clone(), b.api.clone());
        cx.spawn(async move |this, cx| {
            let (api2, paths2) = (api.clone(), paths.clone());
            let result = api::call_api(&rt, move || async move { api2.delete_local_files(&paths2).await }).await;
            let _ = this.update(cx, |this, cx| {
                match result {
                    Ok(data) => {
                        let _ = this.window_handle.update(cx, |_, w, cx| w.close_all_dialogs(cx));
                        this.toast(cx, ToastKind::Success, format!("已删除 {} 项", data.deleted_count));
                        let dir = this.table.read(cx).delegate().current_dir.clone();
                        this.navigate(dir, cx);
                    }
                    Err(e) => this.toast(cx, ToastKind::Error, e.message()),
                }
            });
        }).detach();
    }
}

impl Render for LocalFilesView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let (dir, count, sel) = {
            let d = self.table.read(cx).delegate();
            (d.current_dir.clone(), d.entries.len(), d.selected.len())
        };
        let toolbar = h_flex().gap_3().items_center().px_4().py_2().border_b_1().border_color(theme.border)
            .child(div().flex_1().child(breadcrumb::render(&dir, |target| {
                cx.listener(move |this, _, _, cx| this.navigate(target.clone(), cx))
            })))
            .child(h_flex().gap_2()
                .when(sel > 0, |this| this.child(
                    Button::new("local-del").danger().icon(IconName::Trash).label("删除")
                        .on_click(cx.listener(|this, _, _, cx| { let f = this.table.read(cx).delegate().selected_entries(); this.open_delete(f, cx); }))
                ))
                .child(Button::new("local-refresh").icon(IconName::RefreshCw).label("刷新")
                    .on_click(cx.listener(|this, _, _, cx| { let d = this.table.read(cx).delegate().current_dir.clone(); this.navigate(d, cx); }))));

        v_flex().flex_1().overflow_hidden()
            .child(toolbar)
            .child(div().flex_1().child(DataTable::new(&self.table).stripe(true)))
            .child(h_flex().justify_between().px_4().py_1().border_t_1().border_color(theme.border).text_color(theme.muted_foreground)
                .child(format!("{count} 项"))
                .child(dir))
    }
}
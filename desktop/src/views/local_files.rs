//! 本地文件页:浏览下载目录,支持列表、网格和搜索。

use std::collections::HashSet;
use std::sync::Arc;

use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::component::dialog::Dialog;
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::menu::{ContextMenuExt as _, DropdownMenu as _, PopupMenu, PopupMenuItem};
use gpui_kit::component::{
    Icon, InteractiveElementExt as _, Sizable as _, WindowExt as _, h_flex, v_flex,
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AnyElement, AnyWindowHandle, AppContext as _, Context, Entity, Focusable, FontWeight, Hsla,
    InteractiveElement as _, IntoElement, ParentElement as _, Render,
    StatefulInteractiveElement as _, Styled as _, Subscription, WeakEntity, Window, div, px, rgb,
};

use crate::api::filesystem::{EntryType, FileEntry};
use crate::api::{self, ApiClient};
use crate::bridge::{EventHub, ToastKind, UiEvent};
use crate::components::{file_browser, task_ui, toast};
use crate::design::{self, Palette};
use crate::state::Backend;
use crate::util;

const PAGE_SIZE: usize = 100;

/// ISO8601 修改时间 → 相对时间,解析失败时回退原文。
fn format_mtime(iso: &str) -> String {
    if iso.is_empty() {
        return "-".into();
    }
    chrono::DateTime::parse_from_rfc3339(iso)
        .map(|t| util::format_relative_time(t.timestamp()))
        .unwrap_or_else(|_| iso.to_string())
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ViewMode {
    Grid,
    List,
}

pub struct LocalFilesView {
    entries: Vec<FileEntry>,
    selected: HashSet<String>,
    active_path: Option<String>,
    current_dir: String,
    /// 下载根目录(面包屑「本地文件」锚点,首次加载时记录)。
    base_dir: String,
    has_more: bool,
    page: usize,
    loading: bool,
    loading_more: bool,
    request_version: u64,
    is_search: bool,
    search_keyword: String,
    search_open: bool,
    search_input: Entity<InputState>,
    view_mode: ViewMode,
    hovered: Option<String>,
    open_menu: Option<String>,
    delete_targets: Vec<FileEntry>,
    window_handle: AnyWindowHandle,
    rt: tokio::runtime::Handle,
    api: Arc<ApiClient>,
    _subscriptions: Vec<Subscription>,
}

impl LocalFilesView {
    pub fn new(hub: Entity<EventHub>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let b = cx.global::<Backend>();
        let (rt, api) = (b.rt.clone(), b.api.clone());
        let search_input = cx.new(|cx| InputState::new(window, cx).placeholder("搜索本地文件"));

        let mut subs = Vec::new();
        subs.push(cx.subscribe_in(&search_input, window, |this, _, ev, _window, cx| match ev {
            InputEvent::PressEnter { .. } => this.do_search(cx),
            InputEvent::Change => {
                if this.search_input.read(cx).value().trim().is_empty() && this.is_search {
                    this.exit_search(cx);
                }
            }
            _ => {}
        }));
        subs.push(cx.subscribe(&hub, |this, _, ev, cx| {
            if matches!(
                ev,
                UiEvent::AccountChanged { .. } | UiEvent::DownloadDirChanged
            ) {
                this.base_dir.clear();
                this.current_dir.clear();
                this.entries.clear();
                this.navigate(String::new(), cx);
            }
        }));

        let mut view = Self {
            entries: Vec::new(),
            selected: HashSet::new(),
            active_path: None,
            current_dir: String::new(),
            base_dir: String::new(),
            has_more: false,
            page: 0,
            loading: false,
            loading_more: false,
            request_version: 0,
            is_search: false,
            search_keyword: String::new(),
            search_open: false,
            search_input,
            view_mode: ViewMode::List,
            hovered: None,
            open_menu: None,
            delete_targets: Vec::new(),
            window_handle: window.window_handle(),
            rt,
            api,
            _subscriptions: subs,
        };
        view.navigate(String::new(), cx);
        view
    }

    pub fn navigate(&mut self, path: String, cx: &mut Context<Self>) {
        self.is_search = false;
        self.search_keyword.clear();
        self.load(&path, false, cx);
    }

    fn load(&mut self, path: &str, append: bool, cx: &mut Context<Self>) {
        if append && (self.loading || self.loading_more || !self.has_more) {
            return;
        }
        self.request_version += 1;
        let version = self.request_version;
        if append {
            self.loading_more = true;
        } else {
            self.loading = true;
            self.loading_more = false;
            self.hovered = None;
            self.open_menu = None;
            self.page = 0;
            self.has_more = true;
        }
        let page = if append { self.page + 1 } else { 0 };
        let path = path.to_string();
        let keyword = self.search_keyword.clone();
        let (rt, api) = (self.rt.clone(), self.api.clone());
        cx.notify();
        cx.spawn(async move |this, cx| {
            let (api2, path2, kw2) = (api.clone(), path.clone(), keyword.clone());
            let result = api::call_api(&rt, move || async move {
                api2.list_local_files(&path2, page, PAGE_SIZE, &kw2).await
            })
            .await;
            let _ = this.update(cx, |this, cx| {
                if this.request_version != version {
                    return;
                }
                this.loading = false;
                this.loading_more = false;
                match result {
                    Ok(data) => {
                        if append {
                            this.entries.extend(data.entries);
                        } else {
                            this.entries = data.entries;
                            this.selected.clear();
                            if this.base_dir.is_empty() {
                                this.base_dir = data.current_path.clone();
                            }
                        }
                        this.current_dir = data.current_path;
                        this.has_more = data.has_more;
                        this.page = data.page;
                    }
                    Err(e) => {
                        let msg = match &e {
                            api::ApiError::Business { code, message } => {
                                crate::api::filesystem::friendly_error(*code, message)
                            }
                            _ => e.message(),
                        };
                        let wh = this.window_handle;
                        let _ = wh.update(cx, |_, w, cx| toast::show(w, cx, ToastKind::Error, msg.clone()));
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn reload(&mut self, cx: &mut Context<Self>) {
        let dir = self.current_dir.clone();
        self.load(&dir, false, cx);
    }

    fn load_more(&mut self, cx: &mut Context<Self>) {
        let dir = self.current_dir.clone();
        self.load(&dir, true, cx);
    }

    fn do_search(&mut self, cx: &mut Context<Self>) {
        let key = self.search_input.read(cx).value().trim().to_string();
        if key.is_empty() {
            return;
        }
        self.is_search = true;
        self.search_keyword = key;
        let dir = self.current_dir.clone();
        self.load(&dir, false, cx);
    }

    fn exit_search(&mut self, cx: &mut Context<Self>) {
        if !self.is_search {
            return;
        }
        self.is_search = false;
        self.search_keyword.clear();
        let dir = self.current_dir.clone();
        self.load(&dir, false, cx);
    }

    fn toggle(&mut self, path: String, checked: bool, cx: &mut Context<Self>) {
        if checked {
            self.selected.insert(path);
        } else {
            self.selected.remove(&path);
        }
        cx.notify();
    }

    fn selected_entries(&self) -> Vec<FileEntry> {
        self.entries
            .iter()
            .filter(|e| self.selected.contains(&e.path))
            .cloned()
            .collect()
    }

    fn toast(&self, cx: &mut Context<Self>, kind: ToastKind, msg: impl Into<String>) {
        let msg = msg.into();
        let wh = self.window_handle;
        let _ = wh.update(cx, |_, w, cx| toast::show(w, cx, kind, msg.clone()));
    }

    fn open_entry(&mut self, e: &FileEntry, cx: &mut Context<Self>) {
        if e.entry_type == EntryType::Directory {
            self.navigate(e.path.clone(), cx);
        } else if let Err(err) = open::that(&e.path) {
            self.toast(cx, ToastKind::Error, format!("打开文件失败: {err}"));
        }
    }

    /// 在访达中显示:文件定位到父目录,目录直接打开。
    fn reveal_entry(&mut self, e: &FileEntry, cx: &mut Context<Self>) {
        let target = if e.entry_type == EntryType::Directory {
            std::path::PathBuf::from(&e.path)
        } else {
            match std::path::Path::new(&e.path).parent() {
                Some(p) if !p.as_os_str().is_empty() => p.to_path_buf(),
                _ => {
                    self.toast(cx, ToastKind::Warning, "无法确定文件所在目录");
                    return;
                }
            }
        };
        if let Err(err) = open::that(&target) {
            self.toast(cx, ToastKind::Error, format!("打开访达失败: {err}"));
        }
    }

    fn open_in_finder(&mut self, cx: &mut Context<Self>) {
        let dir = self.current_dir.clone();
        if dir.is_empty() {
            return;
        }
        if let Err(err) = open::that(&dir) {
            self.toast(cx, ToastKind::Error, format!("打开访达失败: {err}"));
        }
    }

    fn open_delete(&mut self, files: Vec<FileEntry>, window: &mut Window, cx: &mut Context<Self>) {
        if files.is_empty() {
            return;
        }
        self.delete_targets = files;
        let count = self.delete_targets.len();
        let view = cx.entity().clone();
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
                        this.reload(cx);
                    }
                    Err(e) => this.toast(cx, ToastKind::Error, e.message()),
                }
            });
        }).detach();
    }

    // ───────────────────────── 顶栏 ─────────────────────────

    fn crumb(
        &self,
        label: String,
        path: String,
        current: bool,
        p: &Palette,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        div()
            .id(gpui_kit::SharedString::from(format!("lc:{path}")))
            .cursor_pointer()
            .truncate()
            .max_w(px(220.))
            .when(current, |d| d.text_color(p.text).font_weight(FontWeight::MEDIUM))
            .when(!current, |d| d.text_color(p.faint).hover(|s| s.text_color(p.muted)))
            .on_click(cx.listener(move |this, _, _, cx| this.navigate(path.clone(), cx)))
            .child(label)
            .into_any_element()
    }

    fn render_breadcrumb(&self, p: &Palette, cx: &mut Context<Self>) -> AnyElement {
        let dir = self.current_dir.clone();
        let base = if self.base_dir.is_empty() { dir.clone() } else { self.base_dir.clone() };
        let mut row = h_flex().gap(px(6.)).items_center().text_size(px(15.));
        row = row.child(self.crumb("本地文件".into(), base.clone(), dir == base, p, cx));
        let rel = dir.strip_prefix(&base).unwrap_or("").to_string();
        let mut acc = base.trim_end_matches('/').to_string();
        let parts: Vec<&str> = rel.split('/').filter(|s| !s.is_empty()).collect();
        for (i, part) in parts.iter().enumerate() {
            acc.push('/');
            acc.push_str(part);
            let current = i + 1 == parts.len();
            row = row.child(Icon::new(IconName::ChevronRight).size(px(14.)).text_color(p.faint));
            row = row.child(self.crumb((*part).to_string(), acc.clone(), current, p, cx));
        }
        row.into_any_element()
    }

    fn tool_icon(
        id: &'static str,
        icon: IconName,
        handler: impl Fn(&mut Self, &mut Window, &mut Context<Self>) + 'static,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        Button::new(id)
            .ghost()
            .small()
            .icon(icon)
            .on_click(cx.listener(move |this, _, window, cx| handler(this, window, cx)))
            .into_any_element()
    }

    fn render_header(&self, p: &Palette, cx: &mut Context<Self>) -> AnyElement {
        let sel = self.selected.len();
        let delete_btn = (!self.loading && sel > 0).then(|| {
            Button::new("local-del")
                .danger()
                .small()
                .icon(IconName::Trash)
                .label(format!("删除 {sel}"))
                .on_click(cx.listener(|this, _, window, cx| {
                    let f = this.selected_entries();
                    this.open_delete(f, window, cx);
                }))
        });

        let search_field = self.search_open.then(|| {
            div().w(px(240.)).child(
                Input::new(&self.search_input)
                    .small()
                    .cleanable(true)
                    .prefix(Icon::new(IconName::Search).size(px(14.)).text_color(p.faint)),
            )
        });

        let search_btn = Self::tool_icon(
            "local-search",
            if self.search_open { IconName::Close } else { IconName::Search },
            move |this, window, cx| {
                this.search_open = !this.search_open;
                if this.search_open {
                    let handle = this.search_input.read(cx).focus_handle(cx);
                    window.focus(&handle, cx);
                } else {
                    let was_search = this.is_search;
                    this.is_search = false;
                    this.search_keyword.clear();
                    this.search_input.update(cx, |s, cx| s.clean(window, cx));
                    if was_search {
                        this.reload(cx);
                    }
                }
                cx.notify();
            },
            cx,
        );
        let refresh_btn = Self::tool_icon("local-refresh", IconName::RefreshCw, move |this, _, cx| {
            this.reload(cx);
        }, cx);
        let view_btn = Self::tool_icon(
            "local-view",
            if self.view_mode == ViewMode::Grid { IconName::LayoutGrid } else { IconName::List },
            move |this, _, cx| {
                this.view_mode = if this.view_mode == ViewMode::Grid { ViewMode::List } else { ViewMode::Grid };
                cx.notify();
            },
            cx,
        );

        file_browser::header(
            self.render_breadcrumb(p, cx),
            (!self.loading).then_some(self.entries.len()),
            self.is_search,
            h_flex()
                .gap(px(6.))
                .items_center()
                .flex_shrink_0()
                .when_some(delete_btn, |this, btn| this.child(btn))
                .when_some(search_field, |this, field| this.child(field))
                .child(view_btn)
                .child(search_btn)
                .child(refresh_btn),
            p,
        )
    }

    // ───────────────────────── 内容区 ─────────────────────────

    fn file_visual(&self, e: &FileEntry, p: &Palette) -> (IconName, Hsla) {
        if e.entry_type == EntryType::Directory {
            (IconName::Folder, rgb(file_browser::FOLDER_ICON_COLOR).into())
        } else {
            let (icon, _, fg) = design::file_visual(&e.name, p.is_dark);
            (icon, fg)
        }
    }

    fn menu_items(
        menu: PopupMenu,
        e: FileEntry,
        view: WeakEntity<LocalFilesView>,
    ) -> PopupMenu {
        let mk = |label: &'static str, icon: IconName, action: LocalAction| {
            let view = view.clone();
            let e = e.clone();
            PopupMenuItem::new(label)
                .icon(icon)
                .on_click(move |_, window, cx| {
                    let e = e.clone();
                    let _ = view.update(cx, |v, cx| match action {
                        LocalAction::Open => v.open_entry(&e, cx),
                        LocalAction::Reveal => v.reveal_entry(&e, cx),
                        LocalAction::Delete => v.open_delete(vec![e.clone()], window, cx),
                    });
                })
        };
        menu.item(mk("打开", IconName::FolderOpen, LocalAction::Open))
            .item(mk("在访达中显示", IconName::Eye, LocalAction::Reveal))
            .item(PopupMenuItem::separator())
            .item(mk("删除", IconName::Trash, LocalAction::Delete))
    }

    fn render_list_row(&self, e: &FileEntry, p: &Palette, cx: &mut Context<Self>) -> AnyElement {
        let path = e.path.clone();
        let is_dir = e.entry_type == EntryType::Directory;
        let name = e.name.clone();
        let selected = self.selected.contains(&path);
        let active = self.active_path.as_deref() == Some(path.as_str());
        let hovered = self.hovered.as_deref() == Some(path.as_str());
        let menu_open = self.open_menu.as_deref() == Some(path.as_str());
        let show_actions = hovered || selected || menu_open;
        let (icon, color) = self.file_visual(e, p);
        let size_text = if is_dir {
            "-".to_string()
        } else {
            e.size.map(util::format_size).unwrap_or_else(|| "-".into())
        };
        let time_text = format_mtime(&e.updated_at);

        // 菜单展开期间保持按钮挂载,避免鼠标移到弹层上时被卸载
        let view_more = cx.entity().downgrade();
        let view_open = cx.entity().downgrade();
        let menu_entry = e.clone();
        let open_path = path.clone();
        let more = Button::new(gpui_kit::SharedString::from(format!("local-more:{path}")))
            .ghost()
            .small()
            .icon(IconName::Ellipsis)
            .dropdown_menu_with_anchor(gpui_kit::Anchor::TopRight, move |menu, _, _| {
                Self::menu_items(menu, menu_entry.clone(), view_more.clone())
            })
            .on_open_change(move |open, _, cx| {
                let open = *open;
                let path = open_path.clone();
                let _ = view_open.update(cx, |v, cx| {
                    if open {
                        v.open_menu = Some(path.clone());
                    } else if v.open_menu.as_deref() == Some(path.as_str()) {
                        v.open_menu = None;
                    }
                    cx.notify();
                });
            });

        let view_sel = cx.entity().downgrade();
        let sel_path = path.clone();
        let checkbox = Checkbox::new(gpui_kit::SharedString::from(format!("local-sel:{path}")))
            .checked(selected)
            .on_change(move |checked, _, cx| {
                let checked = *checked;
                let path = sel_path.clone();
                let _ = view_sel.update(cx, move |v, cx| v.toggle(path, checked, cx));
            });

        let hover_path = path.clone();
        let click_path = path.clone();
        let dbl_entry = e.clone();
        let ctx_entry = e.clone();
        let view_ctx = cx.entity().downgrade();

        div()
            .id(gpui_kit::SharedString::from(format!("local-row:{path}")))
            .h(px(48.))
            .px(px(12.))
            .rounded(px(10.))
            .cursor_pointer()
            .when(hovered || selected || active, |d| d.bg(p.selected_bg))
            .on_hover(cx.listener(move |this, hovered, _, cx| {
                let path = hover_path.clone();
                let next = if *hovered {
                    Some(path.clone())
                } else if this.hovered.as_deref() == Some(path.as_str()) {
                    None
                } else {
                    this.hovered.clone()
                };
                if this.hovered != next {
                    this.hovered = next;
                    cx.notify();
                }
            }))
            .on_click(cx.listener(move |this, _, _, cx| {
                this.active_path = Some(click_path.clone());
                cx.notify();
            }))
            .on_double_click(cx.listener(move |this, _, _, cx| {
                this.open_entry(&dbl_entry, cx);
            }))
            .child(
                h_flex()
                    .items_center()
                    .gap(px(10.))
                    .size_full()
                    .child(
                        div().w(px(28.)).child(if show_actions {
                            checkbox.into_any_element()
                        } else {
                            div().into_any_element()
                        }),
                    )
                    .child(Icon::new(icon).size(px(22.)).text_color(color))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .text_size(px(14.))
                            .font_weight(FontWeight::MEDIUM)
                            .child(name),
                    )
                    .child(
                        div()
                            .w(px(120.))
                            .text_center()
                            .text_size(px(12.))
                            .text_color(p.faint)
                            .child(size_text),
                    )
                    .child(
                        div()
                            .w(px(170.))
                            .pl(px(28.))
                            .text_size(px(12.))
                            .text_color(p.faint)
                            .child(time_text),
                    )
                    .child(
                        div().w(px(36.)).flex().justify_end().child(if show_actions {
                            more.into_any_element()
                        } else {
                            div().into_any_element()
                        }),
                    ),
            )
            .context_menu(move |menu, _, _| {
                Self::menu_items(menu, ctx_entry.clone(), view_ctx.clone())
            })
            .into_any_element()
    }

    fn render_card(&self, e: &FileEntry, p: &Palette, cx: &mut Context<Self>) -> AnyElement {
        let path = e.path.clone();
        let is_dir = e.entry_type == EntryType::Directory;
        let name = e.name.clone();
        let selected = self.selected.contains(&path);
        let active = self.active_path.as_deref() == Some(path.as_str());
        let hovered = self.hovered.as_deref() == Some(path.as_str());
        let menu_open = self.open_menu.as_deref() == Some(path.as_str());
        let show_actions = hovered || selected || menu_open;

        let tile: AnyElement = if is_dir {
            div()
                .size(px(56.))
                .rounded(px(14.))
                .bg(p.folder_tile(false))
                .flex()
                .items_center()
                .justify_center()
                .child(Icon::new(IconName::Folder).size(px(28.)).text_color(gpui_kit::rgb(0xFFFFFF).alpha(0.92)))
                .into_any_element()
        } else {
            let (icon, bg, fg) = design::file_visual(&name, p.is_dark);
            div()
                .size(px(56.))
                .rounded(px(14.))
                .bg(bg)
                .flex()
                .items_center()
                .justify_center()
                .child(Icon::new(icon).size(px(26.)).text_color(fg))
                .into_any_element()
        };

        let meta = if is_dir {
            format_mtime(&e.updated_at)
        } else {
            format!(
                "{} · {}",
                e.size.map(util::format_size).unwrap_or_else(|| "-".into()),
                format_mtime(&e.updated_at)
            )
        };

        let view_sel = cx.entity().downgrade();
        let sel_path = path.clone();
        let checkbox = Checkbox::new(gpui_kit::SharedString::from(format!("local-card-sel:{path}")))
            .checked(selected)
            .on_change(move |checked, _, cx| {
                let checked = *checked;
                let path = sel_path.clone();
                let _ = view_sel.update(cx, move |v, cx| v.toggle(path, checked, cx));
            });

        let view_more = cx.entity().downgrade();
        let view_open = cx.entity().downgrade();
        let menu_entry = e.clone();
        let open_path = path.clone();
        let more = Button::new(gpui_kit::SharedString::from(format!("local-card-more:{path}")))
            .ghost()
            .small()
            .icon(IconName::Ellipsis)
            .dropdown_menu_with_anchor(gpui_kit::Anchor::TopRight, move |menu, _, _| {
                Self::menu_items(menu, menu_entry.clone(), view_more.clone())
            })
            .on_open_change(move |open, _, cx| {
                let open = *open;
                let path = open_path.clone();
                let _ = view_open.update(cx, |v, cx| {
                    if open {
                        v.open_menu = Some(path.clone());
                    } else if v.open_menu.as_deref() == Some(path.as_str()) {
                        v.open_menu = None;
                    }
                    cx.notify();
                });
            });

        let hover_path = path.clone();
        let click_path = path.clone();
        let dbl_entry = e.clone();
        let ctx_entry = e.clone();
        let view_ctx = cx.entity().downgrade();

        div()
            .id(gpui_kit::SharedString::from(format!("local-card:{path}")))
            .relative()
            .w(px(200.))
            .h(px(176.))
            .rounded(px(16.))
            .bg(p.card)
            .border(px(1.))
            .border_color(if selected || active { p.primary } else { p.border })
            .shadow(p.card_shadow(hovered))
            .cursor_pointer()
            .on_hover(cx.listener(move |this, hovered, _, cx| {
                let path = hover_path.clone();
                let next = if *hovered {
                    Some(path.clone())
                } else if this.hovered.as_deref() == Some(path.as_str()) {
                    None
                } else {
                    this.hovered.clone()
                };
                if this.hovered != next {
                    this.hovered = next;
                    cx.notify();
                }
            }))
            .on_click(cx.listener(move |this, _, _, cx| {
                this.active_path = Some(click_path.clone());
                cx.notify();
            }))
            .on_double_click(cx.listener(move |this, _, _, cx| {
                this.open_entry(&dbl_entry, cx);
            }))
            .child(
                v_flex()
                    .size_full()
                    .items_center()
                    .justify_center()
                    .gap(px(10.))
                    .px(px(18.))
                    .child(tile)
                    .child(
                        div()
                            .w_full()
                            .text_center()
                            .truncate()
                            .text_size(px(14.))
                            .font_weight(FontWeight::MEDIUM)
                            .child(name),
                    )
                    .child(
                        div()
                            .text_size(px(12.))
                            .text_color(p.faint)
                            .child(meta),
                    ),
            )
            .when(show_actions, |d| {
                d.child(
                    div()
                        .absolute()
                        .top(px(10.))
                        .left(px(10.))
                        .p(px(1.))
                        .rounded(px(6.))
                        .bg(p.card)
                        .child(checkbox),
                )
            })
            .when(show_actions, |d| {
                d.child(div().absolute().top(px(8.)).right(px(8.)).child(more))
            })
            .context_menu(move |menu, _, _| {
                Self::menu_items(menu, ctx_entry.clone(), view_ctx.clone())
            })
            .into_any_element()
    }

    fn render_empty(&self, p: &Palette, cx: &mut Context<Self>) -> AnyElement {
        if self.is_search {
            return task_ui::empty_state(
                p,
                IconName::FileSearch,
                format!("没有找到包含“{}”的文件", self.search_keyword),
                "换个关键词试试，或清除搜索查看全部文件",
                Some(
                    Button::new("local-clear-search")
                        .small()
                        .icon(IconName::X)
                        .label("清除搜索")
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.search_input.update(cx, |s, cx| s.clean(window, cx));
                            this.exit_search(cx);
                        }))
                        .into_any_element(),
                ),
            );
        }
        task_ui::empty_state(
            p,
            IconName::FolderOpen,
            "这里还没有文件",
            "下载完成的文件会出现在这里",
            Some(
                Button::new("local-open-finder")
                    .small()
                    .icon(IconName::FolderOpen)
                    .label("打开访达")
                    .on_click(cx.listener(|this, _, _, cx| this.open_in_finder(cx)))
                    .into_any_element(),
            ),
        )
    }
}

#[derive(Clone, Copy)]
enum LocalAction {
    Open,
    Reveal,
    Delete,
}

impl Render for LocalFilesView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = design::palette(cx);
        let header = self.render_header(&p, cx);

        let mut body = div()
            .relative()
            .flex_1()
            .min_h_0()
            .overflow_hidden()
            .flex()
            .flex_col();

        if self.loading {
            let label = if self.is_search { "正在搜索…" } else { "正在加载文件夹…" };
            body = body.child(file_browser::loading_state(&p, label));
        } else if self.entries.is_empty() {
            body = body.child(self.render_empty(&p, cx));
        } else {
            let mut scroll = file_browser::scroll("local-scroll");
            match self.view_mode {
                ViewMode::List => {
                    let mut rows = v_flex().gap(px(2.)).child(file_browser::list_header(&p));
                    for e in &self.entries {
                        rows = rows.child(self.render_list_row(e, &p, cx));
                    }
                    scroll = scroll.child(rows);
                }
                ViewMode::Grid => {
                    let mut grid = div().flex().flex_wrap().gap(px(24.));
                    for e in &self.entries {
                        grid = grid.child(self.render_card(e, &p, cx));
                    }
                    scroll = scroll.child(grid);
                }
            }
            if self.has_more {
                scroll = scroll.child(
                    h_flex().justify_center().py(px(24.)).child(
                        Button::new("local-load-more")
                            .small()
                            .label(if self.loading_more { "加载中…" } else { "加载更多" })
                            .on_click(cx.listener(|this, _, _, cx| this.load_more(cx))),
                    ),
                );
            }
            body = body.child(scroll);
        }

        let path = div()
            .px(px(32.))
            .pb(px(12.))
            .flex_shrink_0()
            .truncate()
            .text_right()
            .text_size(px(12.))
            .text_color(p.faint)
            .child(self.current_dir.clone());

        v_flex()
            .flex_1()
            .h_full()
            .min_w_0()
            .overflow_hidden()
            .child(header)
            .child(body)
            .when(!self.current_dir.is_empty(), |page| page.child(path))
    }
}

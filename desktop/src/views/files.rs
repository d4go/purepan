//! 网盘文件浏览页(移植 frontend/src/views/FilesView.vue)。
//! 表格用 gpui-kit DataTable + 自定义 TableDelegate(多选/排序/无限滚动/右键菜单)。

use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;

use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::menu::{PopupMenu, PopupMenuItem};
use gpui_kit::component::dialog::Dialog;
use gpui_kit::component::table::{Column, ColumnSort, DataTable, TableDelegate, TableState};
use gpui_kit::component::tag::Tag;
use gpui_kit::component::{
    ActiveTheme as _, Icon, InteractiveElementExt as _, Sizable as _, WindowExt as _, h_flex, v_flex,
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AnyWindowHandle, App, AppContext as _, BorrowAppContext as _, Context, Div, Entity,
    InteractiveElement as _, IntoElement, ParentElement as _, Render, SharedString, Stateful,
    StatefulInteractiveElement as _, Styled as _, Subscription, Task, WeakEntity, Window, div, px,
};

use crate::api::file::{self, FileItemWithEncryption};
use crate::api::{self, ApiClient};
use crate::bridge::{EventHub, ToastKind, UiEvent};
use crate::components::{breadcrumb, toast};
use crate::state::Backend;
use crate::util;

const PAGE_SIZE: u32 = 50;
const SEARCH_PAGE_SIZE: u32 = 100;
const MAX_WARMUP_RETRIES: u32 = 120;

// ───────────────────────── 表格 Delegate ─────────────────────────

pub struct FilesDelegate {
    pub files: Vec<FileItemWithEncryption>,
    pub selected: HashSet<u64>,
    pub active_fs_id: Option<u64>,
    pub current_dir: String,
    pub has_more: bool,
    pub loading: bool,
    pub loading_more: bool,
    pub warming_up: bool,
    pub is_search: bool,
    pub search_keyword: String,
    page: u32,
    search_page: u32,
    search_has_more: bool,
    sort: Option<(usize, ColumnSort)>,
    request_version: u64,
    warmup_retries: u32,
    rt: tokio::runtime::Handle,
    api: Arc<ApiClient>,
    view: WeakEntity<FilesView>,
}

impl FilesDelegate {
    fn new(rt: tokio::runtime::Handle, api: Arc<ApiClient>, view: WeakEntity<FilesView>) -> Self {
        Self {
            files: Vec::new(),
            selected: HashSet::new(),
            active_fs_id: None,
            current_dir: "/".into(),
            has_more: true,
            loading: false,
            loading_more: false,
            warming_up: false,
            is_search: false,
            search_keyword: String::new(),
            page: 1,
            search_page: 1,
            search_has_more: false,
            sort: None,
            request_version: 0,
            warmup_retries: 0,
            rt,
            api,
            view,
        }
    }

    fn notify_view(&self, cx: &mut App) {
        let _ = self.view.update(cx, |_, cx| cx.notify());
    }

    /// 加载目录(append=false 切目录/刷新;true 滚动下一页)。
    pub fn load_files(&mut self, dir: &str, append: bool, cx: &mut Context<TableState<Self>>) {
        if append && (self.loading || self.loading_more || !self.hasMore()) {
            return;
        }
        self.request_version += 1;
        let version = self.request_version;
        if append {
            self.loading_more = true;
        } else {
            self.loading = true;
            self.page = 1;
            self.has_more = true;
            self.warming_up = false;
            self.warmup_retries = 0;
        }
        let request_page = if append { self.page + 1 } else { 1 };
        let dir = dir.to_string();
        let (rt, api) = (self.rt.clone(), self.api.clone());
        cx.notify();

        cx.spawn(async move |this, cx| {
            let (api2, dir2) = (api.clone(), dir.clone());
            let result = api::call_api(&rt, move || async move {
                api2.file_list(&dir2, request_page, PAGE_SIZE).await
            })
            .await;
            let _ = this.update(cx, |state, cx| {
                let d = state.delegate_mut();
                if d.request_version != version {
                    return;
                }
                match result {
                    Ok(data) => {
                        d.warming_up = false;
                        d.warmup_retries = 0;
                        if append {
                            d.files.extend(data.list);
                        } else {
                            d.files = data.list;
                            d.current_dir = dir.clone();
                            d.selected.clear();
                        }
                        d.has_more = data.has_more;
                        d.page = data.page;
                        d.apply_sort();
                    }
                    Err(e) => {
                        d.has_more = false;
                        let msg = e.message();
                        if !append && msg.contains("未登录或客户端未初始化") {
                            d.warming_up = true;
                            d.warmup_retries += 1;
                            if d.warmup_retries > MAX_WARMUP_RETRIES {
                                d.warming_up = false;
                                d.warmup_retries = 0;
                                d.toast(cx, ToastKind::Error, "客户端初始化超时,请稍后手动刷新重试");
                            } else {
                                d.schedule_warmup_retry(dir.clone(), cx);
                            }
                        } else {
                            d.toast(cx, ToastKind::Error, msg);
                        }
                    }
                }
                d.loading = false;
                d.loading_more = false;
                cx.notify();
                d.notify_view(cx);
            });
        })
        .detach();
    }

    fn schedule_warmup_retry(&mut self, dir: String, cx: &mut Context<TableState<Self>>) {
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(Duration::from_millis(500)).await;
            let _ = this.update(cx, |state, cx| {
                if state.delegate().warming_up {
                    state.delegate_mut().load_files(&dir, false, cx);
                }
            });
        })
        .detach();
    }

    pub fn load_more_search(&mut self, cx: &mut Context<TableState<Self>>) {
        if self.loading || self.loading_more || !self.search_has_more {
            return;
        }
        self.loading_more = true;
        let next_page = self.search_page + 1;
        let key = self.search_keyword.clone();
        let (rt, api) = (self.rt.clone(), self.api.clone());
        cx.notify();
        cx.spawn(async move |this, cx| {
            let (api2, key2) = (api.clone(), key.clone());
            let result = api::call_api(&rt, move || async move {
                api2.search_files(&key2, next_page, SEARCH_PAGE_SIZE, 1).await
            })
            .await;
            let _ = this.update(cx, |state, cx| {
                let d = state.delegate_mut();
                d.loading_more = false;
                if let Ok(data) = result {
                    d.files.extend(data.list);
                    d.search_has_more = data.has_more;
                    d.search_page = next_page;
                    d.apply_sort();
                }
                cx.notify();
                d.notify_view(cx);
            });
        })
        .detach();
    }

    pub fn search(&mut self, key: &str, cx: &mut Context<TableState<Self>>) {
        self.is_search = true;
        self.search_keyword = key.to_string();
        self.search_page = 1;
        self.loading = true;
        self.files.clear();
        self.selected.clear();
        let key = key.to_string();
        let (rt, api) = (self.rt.clone(), self.api.clone());
        cx.notify();
        cx.spawn(async move |this, cx| {
            let (api2, key2) = (api.clone(), key.clone());
            let result = api::call_api(&rt, move || async move {
                api2.search_files(&key2, 1, SEARCH_PAGE_SIZE, 1).await
            })
            .await;
            let _ = this.update(cx, |state, cx| {
                let d = state.delegate_mut();
                d.loading = false;
                match result {
                    Ok(data) => {
                        d.files = data.list;
                        d.search_has_more = data.has_more;
                        d.apply_sort();
                    }
                    Err(e) => d.toast(cx, ToastKind::Error, e.message()),
                }
                cx.notify();
                d.notify_view(cx);
            });
        })
        .detach();
    }

    pub fn exit_search(&mut self, cx: &mut Context<TableState<Self>>) {
        self.is_search = false;
        self.search_keyword.clear();
        let dir = self.current_dir.clone();
        self.load_files(&dir, false, cx);
    }

    fn hasMore(&self) -> bool {
        if self.is_search { self.search_has_more } else { self.has_more }
    }

    fn apply_sort(&mut self) {
        let Some((col, order)) = self.sort else { return };
        if order == ColumnSort::Default {
            return;
        }
        let asc = order == ColumnSort::Ascending;
        self.files.sort_by(|a, b| {
            let ord = match col {
                1 => util::display_name(a).cmp(&util::display_name(b)),
                2 => a.file.size.cmp(&b.file.size),
                3 => a.file.server_mtime.cmp(&b.file.server_mtime),
                _ => std::cmp::Ordering::Equal,
            };
            if asc { ord } else { ord.reverse() }
        });
    }

    fn toast(&self, cx: &mut App, kind: ToastKind, msg: impl Into<String>) {
        let msg = msg.into();
        let _ = self.view.update(cx, |v, cx| v.toast(cx, kind, msg));
    }

    pub fn selected_files(&self) -> Vec<FileItemWithEncryption> {
        self.files
            .iter()
            .filter(|f| self.selected.contains(&f.file.fs_id))
            .cloned()
            .collect()
    }
}

impl TableDelegate for FilesDelegate {
    fn columns_count(&self, _: &App) -> usize {
        5
    }

    fn rows_count(&self, _: &App) -> usize {
        self.files.len()
    }

    fn column(&self, col_ix: usize, _: &App) -> Column {
        match col_ix {
            0 => Column::new("sel", "").width(px(48.)),
            1 => Column::new("name", "文件名").sortable().min_width(px(320.)),
            2 => Column::new("size", "大小").sortable().width(px(100.)),
            3 => Column::new("mtime", "修改时间").sortable().width(px(160.)),
            4 => Column::new("type", "类型").width(px(100.)),
            _ => Column::new("x", ""),
        }
    }

    fn perform_sort(
        &mut self,
        col_ix: usize,
        sort: ColumnSort,
        _window: &mut Window,
        cx: &mut Context<TableState<Self>>,
    ) {
        self.sort = Some((col_ix, sort));
        self.apply_sort();
        cx.notify();
    }

    fn render_tr(&mut self, row_ix: usize, _window: &mut Window, cx: &mut Context<TableState<Self>>) -> Stateful<Div> {
        let theme = cx.theme().clone();
        let view = self.view.clone();
        let Some((fs_id, is_dir, path)) = self.files.get(row_ix)
            .map(|f| (f.file.fs_id, f.file.isdir == 1, f.file.path.clone()))
        else {
            return div().id(("file-row", row_ix));
        };
        let active = self.active_fs_id == Some(fs_id);
        let view_click = view.clone();
        let view_dbl = view.clone();
        div()
            .id(("file-row", row_ix))
            .when(active, |d| d.bg(theme.accent))
            // 桌面端习惯:单击选中,双击打开目录 / 下载文件
            .on_click(move |_, _, cx| {
                let _ = view_click.update(cx, |v, cx| {
                    v.table.update(cx, |state, cx| {
                        state.delegate_mut().active_fs_id = Some(fs_id);
                        cx.notify();
                    });
                });
            })
            .on_double_click(move |_, window, cx| {
                if is_dir {
                    let _ = view_dbl.update(cx, |v, cx| v.navigate_to_dir(path.clone(), cx));
                } else {
                    let _ = view_dbl.update(cx, |v, cx| {
                        let f = v.delegate(cx).files.iter().find(|f| f.file.fs_id == fs_id).cloned();
                        if let Some(f) = f {
                            v.download_files(vec![f], window, cx);
                        }
                    });
                }
            })
    }

    fn context_menu(
        &mut self,
        row_ix: usize,
        menu: PopupMenu,
        _window: &mut Window,
        _cx: &mut Context<TableState<Self>>,
    ) -> PopupMenu {
        let Some(f) = self.files.get(row_ix).cloned() else { return menu };
        let view = self.view.clone();
        let mk = |label: &'static str, icon: IconName, action: FileAction| {
            let view = view.clone();
            let f = f.clone();
            PopupMenuItem::new(label)
                .icon(icon)
                .on_click(move |_, window, cx| {
                    let f = f.clone();
                    let _ = view.update(cx, |v, cx| v.handle_file_action(action, f, window, cx));
                })
        };
        menu.item(mk("打开", IconName::FolderOpen, FileAction::Open))
            .item(mk("下载", IconName::Download, FileAction::Download))
            .item(mk("分享", IconName::Share2, FileAction::Share))
            .item(mk("重命名", IconName::Pencil, FileAction::Rename))
            .item(mk("复制到…", IconName::Copy, FileAction::Copy))
            .item(mk("移动到…", IconName::Move, FileAction::Move))
            .item(PopupMenuItem::separator())
            .item(mk("删除", IconName::Trash, FileAction::Delete))
    }

    fn render_td(
        &mut self,
        row_ix: usize,
        col_ix: usize,
        _window: &mut Window,
        cx: &mut Context<TableState<Self>>,
    ) -> impl IntoElement {
        let Some(f) = self.files.get(row_ix).cloned() else { return div().into_any_element() };
        let theme = cx.theme().clone();
        match col_ix {
            0 => {
                let fs_id = f.file.fs_id;
                let checked = self.selected.contains(&fs_id);
                let view = self.view.clone();
                div()
                    .flex()
                    .items_center()
                    .justify_center()
                    .on_mouse_down(gpui_kit::MouseButton::Left, |_, window, cx| {
                        window.prevent_default();
                        cx.stop_propagation();
                    })
                    .child(
                        Checkbox::new(("sel", row_ix))
                            .checked(checked)
                            .on_change(move |checked, _, cx| {
                                let checked = *checked;
                                let _ = view.update(cx, |v, cx| {
                                    v.toggle_select(fs_id, checked, cx);
                                });
                            }),
                    )
                    .into_any_element()
            }
            1 => {
                let icon = if f.file.isdir == 1 { IconName::Folder } else { IconName::FileText };
                let name = util::display_name(&f);
                let encrypted = f.is_encrypted || f.is_encrypted_folder;
                h_flex()
                    .gap_2()
                    .items_center()
                    .child(Icon::new(icon).size(px(20.)).text_color(if f.file.isdir == 1 { theme.primary } else { theme.muted_foreground }))
                    .child(div().truncate().child(name))
                    .when(encrypted, |this| this.child(Tag::new().small().child("🔒")))
                    .into_any_element()
            }
            2 => div()
                .w_full()
                .text_right()
                .child(if f.file.isdir == 1 { "-".to_string() } else { util::format_size(f.file.size) })
                .into_any_element(),
            3 => div().child(util::format_timestamp(f.file.server_mtime)).into_any_element(),
            4 => div().child(if f.file.isdir == 1 { "文件夹".to_string() } else { util::file_type(&f.file.server_filename).to_string() }).into_any_element(),
            _ => div().into_any_element(),
        }
    }

    fn has_more(&self, _: &App) -> bool {
        self.hasMore()
    }

    fn load_more_threshold(&self) -> usize {
        10
    }

    fn load_more(&mut self, _window: &mut Window, cx: &mut Context<TableState<Self>>) {
        if self.is_search {
            self.load_more_search(cx);
        } else {
            let dir = self.current_dir.clone();
            self.load_files(&dir, true, cx);
        }
    }

    fn loading(&self, _: &App) -> bool {
        self.loading
    }

    fn render_empty(&mut self, _window: &mut Window, cx: &mut Context<TableState<Self>>) -> impl IntoElement {
        let msg = if self.is_search { "未找到匹配的文件" } else { "当前目录为空" };
        h_flex()
            .size_full()
            .justify_center()
            .items_center()
            .text_color(cx.theme().muted_foreground)
            .child(msg)
            .into_any_element()
    }
}

#[derive(Clone, Copy)]
pub enum FileAction {
    Open,
    Download,
    Share,
    Rename,
    Copy,
    Move,
    Delete,
}

// ───────────────────────── 页面视图 ─────────────────────────

pub struct FilesView {
    pub table: Entity<TableState<FilesDelegate>>,
    search_input: Entity<InputState>,
    new_folder_input: Entity<InputState>,
    rename_input: Entity<InputState>,
    rename_target: Option<FileItemWithEncryption>,
    delete_targets: Vec<FileItemWithEncryption>,
    busy: bool,
    hub: Entity<EventHub>,
    window_handle: AnyWindowHandle,
    _subscriptions: Vec<Subscription>,
    _load_task: Option<Task<()>>,
}

impl FilesView {
    pub fn new(hub: Entity<EventHub>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let b = cx.global::<Backend>();
        let (rt, api) = (b.rt.clone(), b.api.clone());
        let view = cx.entity().downgrade();
        let table = cx.new(|cx| TableState::new(FilesDelegate::new(rt, api, view), window, cx));

        let search_input = cx.new(|cx| InputState::new(window, cx).placeholder("搜索"));
        let new_folder_input = cx.new(|cx| InputState::new(window, cx).placeholder("请输入文件夹名称"));
        let rename_input = cx.new(|cx| InputState::new(window, cx).placeholder("输入新名称"));

        let mut subs = Vec::new();
        subs.push(cx.subscribe_in(&search_input, window, |this, _, ev, _, cx| {
            if matches!(ev, InputEvent::PressEnter { .. }) {
                this.do_search(cx);
            }
        }));
        // 账号切换 → 回到根目录重取
        subs.push(cx.subscribe(&hub, |this, _, ev, cx| {
            if matches!(ev, UiEvent::AccountChanged { .. }) {
                this.navigate_to_dir("/".into(), cx);
            }
        }));

        let mut view = Self {
            table,
            search_input,
            new_folder_input,
            rename_input,
            rename_target: None,
            delete_targets: Vec::new(),
            busy: false,
            hub,
            window_handle: window.window_handle(),
            _subscriptions: subs,
            _load_task: None,
        };
        view.navigate_to_dir("/".into(), cx);
        view
    }

    fn delegate<'a>(&self, cx: &'a App) -> &'a FilesDelegate {
        self.table.read(cx).delegate()
    }

    pub fn navigate_to_dir(&mut self, dir: String, cx: &mut Context<Self>) {
        // 退出搜索态
        self.table.update(cx, |state, cx| {
            let d = state.delegate_mut();
            d.is_search = false;
            d.search_keyword.clear();
            d.load_files(&dir, false, cx);
        });
        cx.notify();
    }

    fn do_search(&mut self, cx: &mut Context<Self>) {
        let key = self.search_input.read(cx).value().trim().to_string();
        if key.is_empty() {
            return;
        }
        self.table.update(cx, |state, cx| state.delegate_mut().search(&key, cx));
        cx.notify();
    }

    fn clear_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.search_input.update(cx, |s, cx| s.clean(window, cx));
        self.table.update(cx, |state, cx| state.delegate_mut().exit_search(cx));
        cx.notify();
    }

    pub fn toggle_select(&mut self, fs_id: u64, checked: bool, cx: &mut Context<Self>) {
        self.table.update(cx, |state, cx| {
            let d = state.delegate_mut();
            if checked { d.selected.insert(fs_id); } else { d.selected.remove(&fs_id); }
            cx.notify();
        });
        cx.notify();
    }

    fn select_all(&mut self, checked: bool, cx: &mut Context<Self>) {
        self.table.update(cx, |state, cx| {
            let d = state.delegate_mut();
            d.selected.clear();
            if checked {
                d.selected.extend(d.files.iter().map(|f| f.file.fs_id));
            }
            cx.notify();
        });
        cx.notify();
    }

    pub fn toast(&self, cx: &mut Context<Self>, kind: ToastKind, msg: impl Into<String>) {
        let msg = msg.into();
        let wh = self.window_handle;
        let _ = wh.update(cx, |_, window, cx| toast::show(window, cx, kind, msg.clone()));
    }

    /// 同步(点击/对话框回调)上下文里不能重入 `AnyWindowHandle::update`,
    /// 必须用回调提供的 window 直接弹 toast。
    pub fn toast_sync(&self, window: &mut Window, cx: &mut Context<Self>, kind: ToastKind, msg: impl Into<String>) {
        toast::show(window, cx, kind, msg.into());
    }

    pub fn handle_file_action(&mut self, action: FileAction, f: FileItemWithEncryption, window: &mut Window, cx: &mut Context<Self>) {
        match action {
            FileAction::Open => {
                if f.file.isdir == 1 {
                    self.navigate_to_dir(f.file.path.clone(), cx);
                }
            }
            FileAction::Rename => self.open_rename_dialog(f, window, cx),
            FileAction::Delete => self.open_delete_confirm(vec![f], window, cx),
            FileAction::Download => self.download_files(vec![f], window, cx),
            FileAction::Copy => self.open_copy_move_picker(false, vec![f], window, cx),
            FileAction::Move => self.open_copy_move_picker(true, vec![f], window, cx),
            FileAction::Share => self.open_share_dialog(vec![f], window, cx),
        }
    }

    // ── 新建文件夹 ──
    fn open_new_folder_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let view = cx.entity().clone();
        let input = self.new_folder_input.clone();
        let dir = self.delegate(cx).current_dir.clone();
        input.update(cx, |s, cx| s.clean(window, cx));
        let view2 = view.clone();
        let input2 = input.clone();
        window.open_dialog(cx, move |dialog: Dialog, _, _| {
            let view3 = view2.clone();
            dialog
                .title("新建文件夹")
                .width(px(500.))
                .child(
                    v_flex().gap_3()
                        .child(h_flex().gap_2().items_center().child("文件夹名").child(Input::new(&input2)))
                        .child(h_flex().gap_2().child("当前路径").child(dir.clone())),
                )
                .footer(
                    h_flex().gap_2().justify_end()
                        .child(Button::new("nf-cancel").label("取消").on_click(|_, w, cx| w.close_dialog(cx)))
                        .child(Button::new("nf-ok").primary().label("创建").on_click(move |_, window, cx| {
                            view3.update(cx, |v, cx| v.create_folder(window, cx));
                        })),
                )
                .on_ok({ let v = view2.clone(); move |_, window, cx| { v.update(cx, |v, cx| v.create_folder(window, cx)); true } })
        });
    }

    fn create_folder(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let name = self.new_folder_input.read(cx).value().trim().to_string();
        if let Some(err) = file::validate_filename(&name) {
            self.toast_sync(window, cx, ToastKind::Error, err);
            return;
        }
        let dir = self.delegate(cx).current_dir.clone();
        let path = file::join_path(&dir, &name);
        let b = cx.global::<Backend>();
        let (rt, api) = (b.rt.clone(), b.api.clone());
        cx.spawn(async move |this, cx| {
            let (api2, path2) = (api.clone(), path.clone());
            let result = api::call_api(&rt, move || async move { api2.create_folder(&path2).await }).await;
            let _ = this.update(cx, |this, cx| {
                match result {
                    Ok(_) => {
                        let _ = this.window_handle.update(cx, |_, w, cx| w.close_all_dialogs(cx));
                        this.toast(cx, ToastKind::Success, "文件夹创建成功");
                        let dir = this.delegate(cx).current_dir.clone();
                        this.navigate_to_dir(dir, cx);
                    }
                    Err(e) => this.toast(cx, ToastKind::Error, e.message()),
                }
            });
        }).detach();
    }

    // ── 重命名 ──
    fn open_rename_dialog(&mut self, f: FileItemWithEncryption, window: &mut Window, cx: &mut Context<Self>) {
        self.rename_target = Some(f.clone());
        let name = util::display_name(&f);
        let input = self.rename_input.clone();
        let view = cx.entity().clone();
        let original = name.clone();
        let dir = file::dirname(&f.file.path);
        input.update(cx, |s, cx| s.set_value(name.clone(), window, cx));
        let view2 = view.clone();
        let input2 = input.clone();
        window.open_dialog(cx, move |dialog: Dialog, _, _| {
            let view3 = view2.clone();
            dialog.title("重命名").width(px(500.))
                .child(v_flex().gap_3()
                    .child(h_flex().gap_2().child("原名称").child(original.clone()))
                    .child(h_flex().gap_2().items_center().child("新名称").child(Input::new(&input2)))
                    .child(h_flex().gap_2().child("所在目录").child(dir.clone())))
                .footer(h_flex().gap_2().justify_end()
                    .child(Button::new("rn-cancel").label("取消").on_click(|_, w, cx| w.close_dialog(cx)))
                    .child(Button::new("rn-ok").primary().label("确定").on_click(move |_, window, cx| {
                        view3.update(cx, |v, cx| v.confirm_rename(window, cx));
                    })))
                .on_ok({ let v = view2.clone(); move |_, window, cx| { v.update(cx, |v, cx| v.confirm_rename(window, cx)); true } })
        });
    }

    fn confirm_rename(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(target) = self.rename_target.clone() else { return };
        let newname = self.rename_input.read(cx).value().trim().to_string();
        if let Some(err) = file::validate_filename(&newname) {
            self.toast_sync(window, cx, ToastKind::Error, err);
            return;
        }
        let item = file::RenameItem { path: target.file.path.clone(), newname, id: target.file.fs_id };
        let b = cx.global::<Backend>();
        let (rt, api) = (b.rt.clone(), b.api.clone());
        cx.spawn(async move |this, cx| {
            let (api2, item2) = (api.clone(), item.clone());
            let result = api::call_api(&rt, move || async move { api2.rename_file(&item2).await }).await;
            let _ = this.update(cx, |this, cx| {
                match result {
                    Ok(file::FileOperationOutcome::Success { .. }) => {
                        let _ = this.window_handle.update(cx, |_, w, cx| w.close_all_dialogs(cx));
                        this.toast(cx, ToastKind::Success, "重命名成功");
                        let dir = this.delegate(cx).current_dir.clone();
                        this.navigate_to_dir(dir, cx);
                    }
                    Ok(file::FileOperationOutcome::Failed { message, .. }) => this.toast(cx, ToastKind::Error, message),
                    Err(e) => this.toast(cx, ToastKind::Error, e.message()),
                }
            });
        }).detach();
    }

    // ── 删除 ──
    fn open_delete_confirm(&mut self, files: Vec<FileItemWithEncryption>, window: &mut Window, cx: &mut Context<Self>) {
        self.delete_targets = files;
        let count = self.delete_targets.len();
        let view = cx.entity().clone();
        let view2 = view.clone();
        window.open_dialog(cx, move |dialog: Dialog, _, _| {
            let view3 = view2.clone();
            dialog.title("删除确认").width(px(460.))
                .child(format!("确定删除选中的 {count} 项吗?删除后将移入回收站。"))
                .footer(h_flex().gap_2().justify_end()
                    .child(Button::new("del-cancel").label("取消").on_click(|_, w, cx| w.close_dialog(cx)))
                    .child(Button::new("del-ok").danger().label("删除").on_click(move |_, _, cx| {
                        view3.update(cx, |v, cx| v.confirm_delete(cx));
                    })))
                .on_ok({ let v = view2.clone(); move |_, _, cx| { v.update(cx, |v, cx| v.confirm_delete(cx)); true } })
        });
    }

    fn confirm_delete(&mut self, cx: &mut Context<Self>) {
        let paths: Vec<String> = self.delete_targets.iter().map(|f| f.file.path.clone()).collect();
        if paths.is_empty() { return; }
        let b = cx.global::<Backend>();
        let (rt, api) = (b.rt.clone(), b.api.clone());
        cx.spawn(async move |this, cx| {
            let (api2, paths2) = (api.clone(), paths.clone());
            let result = api::call_api(&rt, move || async move { api2.delete_files(&paths2).await }).await;
            let _ = this.update(cx, |this, cx| {
                match result {
                    Ok(data) => {
                        let _ = this.window_handle.update(cx, |_, w, cx| w.close_all_dialogs(cx));
                        if data.failed_paths.is_empty() {
                            this.toast(cx, ToastKind::Success, format!("已删除 {} 项", data.deleted_count));
                        } else {
                            this.toast(cx, ToastKind::Warning, format!("删除 {} 项,失败 {} 项", data.deleted_count, data.failed_paths.len()));
                        }
                        let dir = this.delegate(cx).current_dir.clone();
                        this.navigate_to_dir(dir, cx);
                    }
                    Err(api::ApiError::Business { code: 132, message }) => {
                        let _ = this.window_handle.update(cx, |_, w, cx| w.close_all_dialogs(cx));
                        this.toast(cx, ToastKind::Error, format!("{message}(触发风控,请前往 pan.baidu.com 验证)"));
                    }
                    Err(e) => this.toast(cx, ToastKind::Error, e.message()),
                }
            });
        }).detach();
    }

    // ── 复制 / 移动 ──
    fn open_copy_move_picker(&mut self, move_op: bool, files: Vec<FileItemWithEncryption>, window: &mut Window, cx: &mut Context<Self>) {
        if files.is_empty() {
            self.toast_sync(window, cx, ToastKind::Warning, "请先选择文件或文件夹");
            return;
        }
        // 禁选:同源父目录(精确);移动时还禁选源文件夹子树
        let mut blocked_exact: Vec<String> = files.iter().map(|f| file::dirname(&f.file.path)).collect();
        blocked_exact.sort();
        blocked_exact.dedup();
        let blocked_paths: Vec<String> = if move_op {
            files.iter().filter(|f| f.file.isdir == 1).map(|f| f.file.path.clone()).collect()
        } else {
            Vec::new()
        };
        let initial = self.delegate(cx).current_dir.clone();
        let view = cx.entity().downgrade();
        let title = if move_op { "选择移动目标文件夹" } else { "选择复制目标文件夹" };
        let on_confirm: std::rc::Rc<dyn Fn(String, &mut Window, &mut App)> = std::rc::Rc::new(move |dest, _window, cx| {
            let files = files.clone();
            let _ = view.update(cx, move |v, cx| v.do_copy_move(move_op, files, dest, cx));
        });
        let picker = cx.new(|cx| {
            crate::components::dialogs::netdisk_picker::NetdiskPicker::new(
                &initial, blocked_paths, blocked_exact, on_confirm, window, cx,
            )
        });
        window.open_dialog(cx, move |dialog: Dialog, _, _| dialog.title(title).width(px(560.)).child(picker.clone()));
    }

    fn do_copy_move(&mut self, move_op: bool, files: Vec<FileItemWithEncryption>, dest: String, cx: &mut Context<Self>) {
        let items: Vec<file::FileOperationItem> = files.iter().map(|f| file::FileOperationItem {
            path: f.file.path.clone(),
            dest: dest.clone(),
            newname: file::basename(&f.file.path),
        }).collect();
        let b = cx.global::<Backend>();
        let (rt, api) = (b.rt.clone(), b.api.clone());
        cx.spawn(async move |this, cx| {
            let result = if move_op {
                let (api2, items2) = (api.clone(), items.clone());
                api::call_api(&rt, move || async move { api2.move_files(&items2).await }).await
            } else {
                let (api2, items2) = (api.clone(), items.clone());
                api::call_api(&rt, move || async move { api2.copy_files(&items2).await }).await
            };
            let _ = this.update(cx, |this, cx| {
                let verb = if move_op { "移动" } else { "复制" };
                match result {
                    Ok(file::FileOperationOutcome::Success { .. }) => {
                        this.toast(cx, ToastKind::Success, format!("{verb}成功"));
                        let dir = this.delegate(cx).current_dir.clone();
                        this.navigate_to_dir(dir, cx);
                    }
                    Ok(file::FileOperationOutcome::Failed { message, still_running, .. }) => {
                        if still_running {
                            this.toast(cx, ToastKind::Info, format!("{verb}任务仍在进行,请稍后刷新查看"));
                        } else {
                            this.toast(cx, ToastKind::Error, message);
                        }
                    }
                    Err(e) => this.toast(cx, ToastKind::Error, e.message()),
                }
            });
        }).detach();
    }

    // ── 下载(支持 ask_each_time 目录选择) ──
    fn download_files(&mut self, files: Vec<FileItemWithEncryption>, window: &mut Window, cx: &mut Context<Self>) {
        if files.is_empty() {
            self.toast_sync(window, cx, ToastKind::Warning, "请先选择要下载的文件或文件夹");
            return;
        }
        let b = cx.global::<Backend>();
        let (rt, api) = (b.rt.clone(), b.api.clone());
        cx.spawn(async move |this, cx| {
            let cfg = api::call_api(&rt, { let api = api.clone(); move || async move { api.get_config().await } }).await;
            let (ask, default_dir) = match cfg {
                Ok(c) => (c.download.ask_each_time, c.download.download_dir.display().to_string()),
                Err(_) => (false, "downloads".into()),
            };
            let _ = this.update(cx, |this, cx| {
                if ask {
                    let view = cx.entity().downgrade();
                    let files2 = files.clone();
                    let on_confirm: std::rc::Rc<dyn Fn(Vec<String>, &mut Window, &mut App)> =
                        std::rc::Rc::new(move |paths, _w, cx| {
                            if let Some(dir) = paths.first() {
                                let dir = dir.clone();
                                let files = files2.clone();
                                let _ = view.update(cx, move |v, cx| v.do_download_batch(files, dir, cx));
                            }
                        });
                    let wh = this.window_handle;
                    let initial = default_dir.clone();
                    let _ = wh.update(cx, move |_, window, cx| {
                        let picker = cx.new(|cx| {
                            crate::components::dialogs::file_picker::LocalFilePicker::new(
                                crate::components::dialogs::file_picker::PickerMode::Directory,
                                Some(initial), on_confirm, window, cx,
                            )
                        });
                        window.open_dialog(cx, move |d: Dialog, _, _| d.title("选择下载目录").width(px(640.)).child(picker.clone()));
                    });
                } else {
                    this.do_download_batch(files.clone(), default_dir, cx);
                }
            });
        }).detach();
    }

    fn do_download_batch(&mut self, files: Vec<FileItemWithEncryption>, dir: String, cx: &mut Context<Self>) {
        let items: Vec<api::download::BatchDownloadItem> = files.iter().map(|f| api::download::BatchDownloadItem {
            fs_id: f.file.fs_id,
            path: f.file.path.clone(),
            name: util::display_name(f),
            is_dir: f.file.isdir == 1,
            size: if f.file.isdir == 1 { None } else { Some(f.file.size) },
            original_name: f.original_name.clone(),
        }).collect();
        let b = cx.global::<Backend>();
        let (rt, api) = (b.rt.clone(), b.api.clone());
        cx.spawn(async move |this, cx| {
            let (api2, items2, dir2) = (api.clone(), items.clone(), dir.clone());
            let result = api::call_api(&rt, move || async move {
                api2.create_downloads_batch(&items2, &dir2, None).await
            }).await;
            let _ = this.update(cx, |this, cx| {
                match result {
                    Ok(resp) => {
                        let ok = resp.task_ids.len() + resp.folder_task_ids.len();
                        if resp.failed.is_empty() {
                            this.toast(cx, ToastKind::Success, format!("已加入下载队列({ok} 项)"));
                        } else {
                            this.toast(cx, ToastKind::Warning, format!("加入 {ok} 项,失败 {} 项", resp.failed.len()));
                        }
                    }
                    Err(e) => this.toast(cx, ToastKind::Error, e.message()),
                }
            });
        }).detach();
    }

    // ── 上传 ──
    fn open_upload_picker(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let view = cx.entity().downgrade();
        let on_confirm: std::rc::Rc<dyn Fn(Vec<String>, &mut Window, &mut App)> =
            std::rc::Rc::new(move |paths, _w, cx| {
                let _ = view.update(cx, move |v, cx| v.upload_paths(paths, cx));
            });
        let picker = cx.new(|cx| {
            crate::components::dialogs::file_picker::LocalFilePicker::new(
                crate::components::dialogs::file_picker::PickerMode::Files,
                None, on_confirm, window, cx,
            )
        });
        window.open_dialog(cx, move |d: Dialog, _, _| d.title("选择上传文件").width(px(640.)).child(picker.clone()));
    }

    fn upload_paths(&mut self, paths: Vec<String>, cx: &mut Context<Self>) {
        if paths.is_empty() { return; }
        let remote_dir = self.delegate(cx).current_dir.clone();
        let b = cx.global::<Backend>();
        let (rt, api) = (b.rt.clone(), b.api.clone());
        cx.spawn(async move |this, cx| {
            let mut ok = 0usize;
            let mut fail = 0usize;
            for path in &paths {
                let is_dir = std::fs::metadata(path).map(|m| m.is_dir()).unwrap_or(false);
                let (api2, path2, remote2) = (api.clone(), path.clone(), remote_dir.clone());
                let result = if is_dir {
                    api::call_api(&rt, move || async move {
                        api2.create_folder_upload(&path2, &remote2, false, None).await.map(|_| ())
                    }).await
                } else {
                    let remote_path = file::join_path(&remote_dir, &file::basename(path));
                    api::call_api(&rt, move || async move {
                        api2.create_upload(&path2, &remote_path, false, None).await.map(|_| ())
                    }).await
                };
                match result { Ok(_) => ok += 1, Err(_) => fail += 1 }
            }
            let _ = this.update(cx, |this, cx| {
                if fail == 0 {
                    this.toast(cx, ToastKind::Success, format!("已加入上传队列({ok} 项)"));
                } else {
                    this.toast(cx, ToastKind::Warning, format!("上传 {ok} 项,失败 {fail} 项"));
                }
            });
        }).detach();
    }
    // ── 分享 / 转存 ──
    fn open_share_dialog(&mut self, files: Vec<FileItemWithEncryption>, window: &mut Window, cx: &mut Context<Self>) {
        if files.is_empty() { self.toast_sync(window, cx, ToastKind::Warning, "请先选择要分享的文件"); return; }
        let paths: Vec<String> = files.iter().map(|f| f.file.path.clone()).collect();
        let dlg = cx.new(|cx| crate::components::dialogs::share_dialog::ShareDialog::new(paths.clone(), window, cx));
        window.open_dialog(cx, move |d: Dialog, _, _| d.title("分享").width(px(560.)).child(dlg.clone()));
    }

    fn open_transfer_dialog(&mut self, direct: bool, window: &mut Window, cx: &mut Context<Self>) {
        let initial = self.delegate(cx).current_dir.clone();
        let view = cx.entity().downgrade();
        let on_success: std::rc::Rc<dyn Fn(&mut Window, &mut App)> = std::rc::Rc::new(move |_w, cx| {
            let _ = view.update(cx, |v, cx| { let dir = v.delegate(cx).current_dir.clone(); v.navigate_to_dir(dir, cx); });
        });
        let dlg = cx.new(|cx| crate::components::dialogs::transfer_dialog::TransferDialog::new(direct, &initial, on_success, window, cx));
        let title = if direct { "分享直下" } else { "转存" };
        window.open_dialog(cx, move |d: Dialog, _, _| d.title(title).width(px(600.)).child(dlg.clone()));
    }
}

impl Render for FilesView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let (dir, is_search, keyword, item_count, sel_count, warming, loading_more, has_more) = {
            let d = self.delegate(cx);
            (d.current_dir.clone(), d.is_search, d.search_keyword.clone(), d.files.len(), d.selected.len(), d.warming_up, d.loading_more, d.hasMore())
        };

        let mut toolbar = h_flex().gap_3().items_center().px_4().py_1p5();
        // 面包屑
        toolbar = toolbar.child(div().flex_1().child(breadcrumb::render(&dir, |target| {
            cx.listener(move |this, _, _, cx| this.navigate_to_dir(target.clone(), cx))
        })));
        // 搜索框
        toolbar = toolbar.child(
            h_flex().gap_1().items_center().w(px(220.))
                .child(Input::new(&self.search_input).small())
                .when(is_search || !keyword.is_empty(), |this| this.child(
                    Button::new("clear-search").ghost().small().icon(IconName::Close)
                        .on_click(cx.listener(|this, _, window, cx| this.clear_search(window, cx)))
                )),
        );
        // 操作按钮
        let actions = if sel_count > 0 {
            h_flex().gap_2()
                .child(Button::new("batch-dl").small().icon(IconName::Download).label("下载").on_click(cx.listener(|this, _, window, cx| { let f = this.delegate(cx).selected_files(); this.download_files(f, window, cx); })))
                .child(Button::new("batch-copy").small().icon(IconName::Copy).label("复制").on_click(cx.listener(|this, _, window, cx| { let f = this.delegate(cx).selected_files(); this.open_copy_move_picker(false, f, window, cx); })))
                .child(Button::new("batch-move").small().icon(IconName::Move).label("移动").on_click(cx.listener(|this, _, window, cx| { let f = this.delegate(cx).selected_files(); this.open_copy_move_picker(true, f, window, cx); })))
                .child(Button::new("batch-share").small().icon(IconName::Share2).label("分享").on_click(cx.listener(|this, _, window, cx| { let f = this.delegate(cx).selected_files(); this.open_share_dialog(f, window, cx); })))
                .child(Button::new("batch-del").danger().small().icon(IconName::Trash).label("删除").on_click(cx.listener(|this, _, window, cx| { let f = this.delegate(cx).selected_files(); this.open_delete_confirm(f, window, cx); })))
        } else {
            h_flex().gap_2()
                .child(Button::new("new-folder").primary().small().icon(IconName::FolderPlus).label("新建").on_click(cx.listener(|this, _, window, cx| this.open_new_folder_dialog(window, cx))))
                .child(Button::new("upload").success().small().icon(IconName::Upload).label("上传").on_click(cx.listener(|this, _, window, cx| this.open_upload_picker(window, cx))))
                .child(Button::new("transfer").warning().small().icon(IconName::FolderInput).label("转存").on_click(cx.listener(|this, _, window, cx| this.open_transfer_dialog(false, window, cx))))
                .child(Button::new("share-direct").warning().small().icon(IconName::Download).label("分享直下").on_click(cx.listener(|this, _, window, cx| this.open_transfer_dialog(true, window, cx))))
                .child(Button::new("refresh").small().icon(IconName::RefreshCw).label("刷新").on_click(cx.listener(|this, _, _, cx| { let dir = this.delegate(cx).current_dir.clone(); this.navigate_to_dir(dir, cx); })))
        };
        toolbar = toolbar.child(actions);

        let mut content = v_flex().flex_1().overflow_hidden().child(toolbar);
        if warming {
            content = content.child(div().flex_1().flex().items_center().justify_center().child("正在初始化客户端..."));
        } else {
            content = content.child(div().flex_1().child(DataTable::new(&self.table).stripe(true)));
        }
        // 状态栏
        let status = h_flex().justify_between().px_4().py_1p5().text_color(theme.muted_foreground).text_size(px(12.))
            .child(h_flex().gap_2().child(format!("{item_count} 项")).when(sel_count > 0, |this| this.child(format!("· 已选择 {sel_count} 项"))))
            .child(h_flex().gap_2().when(is_search, |this| this.child(format!("搜索结果: \"{keyword}\""))).child(dir.clone()));
        content = content.child(status);
        let _ = (loading_more, has_more);
        content
    }
}
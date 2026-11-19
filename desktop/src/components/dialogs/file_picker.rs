//! 本地文件选择器(移植 FilePickerModal 的核心:路径导航 + 列表选择 + 分页)。
//! Files 模式多选文件/文件夹(上传用);Directory 模式选择当前目录(下载目标用)。

use std::collections::HashSet;
use std::rc::Rc;
use std::sync::Arc;

use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::{ActiveTheme as _, Disableable as _, Icon, Sizable as _, WindowExt as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AnyWindowHandle, App, AppContext as _, Context, Entity, InteractiveElement as _, IntoElement,
    ParentElement as _, Render, StatefulInteractiveElement as _, Styled as _, Subscription, Window,
    div, px,
};

use crate::api::filesystem::{EntryType, FileEntry};
use crate::api::{self, ApiClient};
use crate::state::Backend;
use crate::util;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum PickerMode {
    Files,
    Directory,
}

type ConfirmFn = Rc<dyn Fn(Vec<String>, &mut Window, &mut App)>;

pub struct LocalFilePicker {
    mode: PickerMode,
    current_path: String,
    parent_path: Option<String>,
    entries: Vec<FileEntry>,
    selected: HashSet<String>,
    has_more: bool,
    page: usize,
    loading: bool,
    loading_more: bool,
    back: Vec<String>,
    forward: Vec<String>,
    path_input: Entity<InputState>,
    rt: tokio::runtime::Handle,
    api: Arc<ApiClient>,
    on_confirm: ConfirmFn,
    window_handle: AnyWindowHandle,
    _subscriptions: Vec<Subscription>,
}

impl LocalFilePicker {
    pub fn new(
        mode: PickerMode,
        initial_path: Option<String>,
        on_confirm: ConfirmFn,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let b = cx.global::<Backend>();
        let (rt, api) = (b.rt.clone(), b.api.clone());
        let path_input = cx.new(|cx| InputState::new(window, cx).placeholder("输入路径后回车"));
        let mut subs = Vec::new();
        subs.push(cx.subscribe_in(&path_input, window, |this, _, ev, _, cx| {
            if matches!(ev, InputEvent::PressEnter { .. }) {
                this.goto_input(cx);
            }
        }));
        let mut picker = Self {
            mode,
            current_path: String::new(),
            parent_path: None,
            entries: Vec::new(),
            selected: HashSet::new(),
            has_more: false,
            page: 0,
            loading: false,
            loading_more: false,
            back: Vec::new(),
            forward: Vec::new(),
            path_input,
            rt,
            api,
            on_confirm,
            window_handle: window.window_handle(),
            _subscriptions: subs,
        };
        picker.init(initial_path, cx);
        picker
    }

    fn init(&mut self, initial: Option<String>, cx: &mut Context<Self>) {
        let (rt, api) = (self.rt.clone(), self.api.clone());
        cx.spawn(async move |this, cx| {
            // 起始路径:优先 initial,否则 roots 的 defaultPath
            let start = if let Some(p) = initial.filter(|p| !p.is_empty()) {
                p
            } else {
                let roots = api::call_api(&rt, { let api = api.clone(); move || async move { api.get_roots().await } }).await;
                roots.ok().and_then(|r| r.default_path).unwrap_or_else(|| "/".into())
            };
            let _ = this.update(cx, |this, cx| {
                this.current_path = start;
                this.load_entries(false, cx);
            });
        }).detach();
    }

    fn load_entries(&mut self, append: bool, cx: &mut Context<Self>) {
        if append && (self.loading || self.loading_more || !self.has_more) {
            return;
        }
        if append { self.loading_more = true; } else { self.loading = true; self.page = 0; }
        let path = self.current_path.clone();
        let page = if append { self.page + 1 } else { 0 };
        let (rt, api) = (self.rt.clone(), self.api.clone());
        cx.notify();
        cx.spawn(async move |this, cx| {
            let (api2, path2) = (api.clone(), path.clone());
            let result = api::call_api(&rt, move || async move {
                api2.list_directory(&path2, page, 100, "name", "asc", None).await
            }).await;
            let _ = this.update(cx, |this, cx| {
                this.loading = false;
                this.loading_more = false;
                match result {
                    Ok(data) => {
                        if append { this.entries.extend(data.entries); } else { this.entries = data.entries; }
                        this.current_path = data.current_path;
                        this.parent_path = data.parent_path;
                        this.has_more = data.has_more;
                        this.page = data.page;
                        this.sync_path_input(cx);
                    }
                    Err(e) => {
                        let msg = match &e {
                            api::ApiError::Business { code, message } => crate::api::filesystem::friendly_error(*code, message),
                            _ => e.message(),
                        };
                        let wh = this.window_handle;
                        let _ = wh.update(cx, |_, w, cx| crate::components::toast::show(w, cx, crate::bridge::ToastKind::Error, msg));
                    }
                }
                cx.notify();
            });
        }).detach();
    }

    fn sync_path_input(&self, cx: &mut Context<Self>) {
        let path = self.current_path.clone();
        let input = self.path_input.clone();
        let wh = self.window_handle;
        let _ = wh.update(cx, |_, w, cx| {
            input.update(cx, |s, cx| s.set_value(path.clone(), w, cx));
        });
    }

    fn navigate(&mut self, path: String, cx: &mut Context<Self>) {
        if !self.current_path.is_empty() {
            self.back.push(self.current_path.clone());
        }
        self.forward.clear();
        self.current_path = path;
        self.selected.clear();
        self.load_entries(false, cx);
    }

    fn go_back(&mut self, cx: &mut Context<Self>) {
        if let Some(p) = self.back.pop() {
            self.forward.push(self.current_path.clone());
            self.current_path = p;
            self.selected.clear();
            self.load_entries(false, cx);
        }
    }

    fn go_forward(&mut self, cx: &mut Context<Self>) {
        if let Some(p) = self.forward.pop() {
            self.back.push(self.current_path.clone());
            self.current_path = p;
            self.selected.clear();
            self.load_entries(false, cx);
        }
    }

    fn go_up(&mut self, cx: &mut Context<Self>) {
        if let Some(p) = self.parent_path.clone() {
            self.navigate(p, cx);
        }
    }

    fn goto_input(&mut self, cx: &mut Context<Self>) {
        let path = self.path_input.read(cx).value().trim().to_string();
        if path.is_empty() { return; }
        let (rt, api) = (self.rt.clone(), self.api.clone());
        cx.spawn(async move |this, cx| {
            let (api2, path2) = (api.clone(), path.clone());
            let result = api::call_api(&rt, move || async move { api2.goto_path(&path2).await }).await;
            let _ = this.update(cx, |this, cx| {
                if let Ok(g) = result {
                    if g.valid {
                        let resolved = g.resolved_path.clone();
                        this.navigate(resolved, cx);
                    }
                }
            });
        }).detach();
    }

    fn toggle(&mut self, path: String, checked: bool, cx: &mut Context<Self>) {
        if checked { self.selected.insert(path); } else { self.selected.remove(&path); }
        cx.notify();
    }

    fn confirm(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let paths = match self.mode {
            PickerMode::Directory => vec![self.current_path.clone()],
            PickerMode::Files => self.selected.iter().cloned().collect(),
        };
        if paths.is_empty() { return; }
        (self.on_confirm)(paths, window, cx);
        window.close_dialog(cx);
    }
}

impl Render for LocalFilePicker {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let mode = self.mode;
        let sel_count = self.selected.len();

        // 导航栏
        let nav = h_flex().gap_1().items_center()
            .child(Button::new("fp-back").ghost().small().icon(IconName::ArrowLeft).disabled(self.back.is_empty()).on_click(cx.listener(|this, _, _, cx| this.go_back(cx))))
            .child(Button::new("fp-fwd").ghost().small().icon(IconName::ArrowRight).disabled(self.forward.is_empty()).on_click(cx.listener(|this, _, _, cx| this.go_forward(cx))))
            .child(Button::new("fp-up").ghost().small().icon(IconName::ArrowUp).disabled(self.parent_path.is_none()).on_click(cx.listener(|this, _, _, cx| this.go_up(cx))))
            .child(Button::new("fp-refresh").ghost().small().icon(IconName::RefreshCw).on_click(cx.listener(|this, _, _, cx| { let p = this.current_path.clone(); this.navigate(p, cx); })))
            .child(div().flex_1().child(Input::new(&self.path_input).large()));

        // 列表
        let picker = cx.entity().downgrade();
        let mut list = v_flex().gap_0p5().max_h(px(360.)).overflow_y_scrollbar();
        if self.loading {
            list = list.child(div().py_4().text_center().child("加载中..."));
        } else if self.entries.is_empty() {
            list = list.child(div().py_4().text_center().text_color(theme.muted_foreground).child("空目录"));
        } else {
            for (ix, e) in self.entries.iter().enumerate() {
                let is_dir = e.entry_type == EntryType::Directory;
                let path = e.path.clone();
                let name = e.name.clone();
                let size = e.size;
                let enter_path = path.clone();
                let checked = self.selected.contains(&path);
                let icon = if is_dir { IconName::Folder } else { IconName::FileText };
                let mut row = h_flex()
                    .id(("fp-entry", ix))
                    .gap_2()
                    .items_center()
                    .px_2()
                    .py_1p5()
                    .rounded_md()
                    .cursor_pointer()
                    .hover(|s| s.bg(theme.secondary));
                if mode == PickerMode::Files {
                    let picker = picker.clone();
                    let sel_path2 = path.clone();
                    row = row.child(
                        div().on_mouse_down(gpui_kit::MouseButton::Left, |_, w, cx| { w.prevent_default(); cx.stop_propagation(); })
                            .child(Checkbox::new(("fp-sel", ix)).checked(checked).on_change(move |c, _, cx| {
                                let c = *c;
                                let p = sel_path2.clone();
                                let _ = picker.update(cx, |this, cx| this.toggle(p, c, cx));
                            })),
                    );
                }
                row = row
                    .child(Icon::new(icon).size(px(18.)).text_color(if is_dir { theme.primary } else { theme.muted_foreground }))
                    .child(div().flex_1().truncate().child(name))
                    .child(div().text_color(theme.muted_foreground).child(if is_dir { "-".to_string() } else { size.map(util::format_size).unwrap_or_else(|| "-".into()) }))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if is_dir {
                            this.navigate(enter_path.clone(), cx);
                        } else if mode == PickerMode::Files {
                            let checked = this.selected.contains(&enter_path);
                            this.toggle(enter_path.clone(), !checked, cx);
                        }
                    }));
                list = list.child(row);
            }
            if self.has_more {
                list = list.child(
                    Button::new("fp-more").ghost().small().label("加载更多")
                        .on_click(cx.listener(|this, _, _, cx| this.load_entries(true, cx))),
                );
            }
        }

        let confirm_label = match mode {
            PickerMode::Directory => "选择此目录",
            PickerMode::Files => "上传",
        };
        let confirm_disabled = match mode {
            PickerMode::Directory => self.current_path.is_empty(),
            PickerMode::Files => sel_count == 0,
        };

        v_flex().gap_3().w(px(560.))
            .child(nav)
            .child(list)
            .child(
                h_flex().gap_2().justify_between().items_center()
                    .child(div().text_color(theme.muted_foreground).child(match mode {
                        PickerMode::Files => format!("已选择 {sel_count} 项"),
                        PickerMode::Directory => self.current_path.clone(),
                    }))
                    .child(h_flex().gap_2()
                        .child(Button::new("fp-cancel").label("取消").on_click(|_, w, cx| w.close_dialog(cx)))
                        .child(Button::new("fp-confirm").primary().label(confirm_label).disabled(confirm_disabled)
                            .on_click(cx.listener(|this, _, window, cx| this.confirm(window, cx))))),
            )
    }
}
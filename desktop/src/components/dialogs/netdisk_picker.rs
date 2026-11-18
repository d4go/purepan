//! 网盘文件夹选择器(移植 NetdiskFolderPickerModal.vue):浏览目录、新建文件夹、
//! 子树/精确禁选,确认后回调目标路径。用于复制/移动目标选择。

use std::rc::Rc;
use std::sync::Arc;

use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::{ActiveTheme as _, Disableable as _, Icon, Sizable as _, WindowExt as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AnyWindowHandle, App, AppContext as _, Context, Entity, InteractiveElement as _, IntoElement,
    ParentElement as _, Render, StatefulInteractiveElement as _, Styled as _, Subscription, Window,
    div, px,
};

use crate::api::file::{self, FileItemWithEncryption};
use crate::api::{self, ApiClient};
use crate::components::breadcrumb;
use crate::state::Backend;

type ConfirmFn = Rc<dyn Fn(String, &mut Window, &mut App)>;

pub struct NetdiskPicker {
    current_path: String,
    folders: Vec<FileItemWithEncryption>,
    blocked_paths: Vec<String>,
    blocked_exact: Vec<String>,
    loading: bool,
    new_folder_input: Entity<InputState>,
    rt: tokio::runtime::Handle,
    api: Arc<ApiClient>,
    on_confirm: ConfirmFn,
    window_handle: AnyWindowHandle,
    _subscriptions: Vec<Subscription>,
}

impl NetdiskPicker {
    pub fn new(
        initial_path: &str,
        blocked_paths: Vec<String>,
        blocked_exact: Vec<String>,
        on_confirm: ConfirmFn,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let b = cx.global::<Backend>();
        let (rt, api) = (b.rt.clone(), b.api.clone());
        let new_folder_input = cx.new(|cx| InputState::new(window, cx).placeholder("新建文件夹名称"));
        let mut subs = Vec::new();
        subs.push(cx.subscribe_in(&new_folder_input, window, |this, _, ev, _, cx| {
            if matches!(ev, InputEvent::PressEnter { .. }) {
                this.create_folder(cx);
            }
        }));
        let mut picker = Self {
            current_path: file::normalize_path(initial_path),
            folders: Vec::new(),
            blocked_paths,
            blocked_exact,
            loading: false,
            new_folder_input,
            rt,
            api,
            on_confirm,
            window_handle: window.window_handle(),
            _subscriptions: subs,
        };
        picker.load_folders(cx);
        picker
    }

    fn load_folders(&mut self, cx: &mut Context<Self>) {
        self.loading = true;
        let dir = self.current_path.clone();
        let (rt, api) = (self.rt.clone(), self.api.clone());
        cx.notify();
        cx.spawn(async move |this, cx| {
            let (api2, dir2) = (api.clone(), dir.clone());
            let result = api::call_api(&rt, move || async move {
                api2.file_list(&dir2, 1, 200).await
            }).await;
            let _ = this.update(cx, |this, cx| {
                this.loading = false;
                if let Ok(data) = result {
                    this.folders = data.list.into_iter().filter(|f| f.file.isdir == 1).collect();
                }
                cx.notify();
            });
        }).detach();
    }

    fn navigate(&mut self, path: String, cx: &mut Context<Self>) {
        self.current_path = file::normalize_path(&path);
        self.load_folders(cx);
    }

    fn is_blocked_selection(&self, path: &str) -> bool {
        let target = file::normalize_path(path);
        if self.blocked_exact.iter().any(|b| file::normalize_path(b) == target) {
            return true;
        }
        self.blocked_paths.iter().any(|b| {
            let b = file::normalize_path(b);
            target == b || target.starts_with(&format!("{b}/"))
        })
    }

    fn is_blocked_entry(&self, path: &str) -> bool {
        let target = file::normalize_path(path);
        self.blocked_paths.iter().any(|b| {
            let b = file::normalize_path(b);
            target == b || target.starts_with(&format!("{b}/"))
        })
    }

    fn create_folder(&mut self, cx: &mut Context<Self>) {
        let name = self.new_folder_input.read(cx).value().trim().to_string();
        if let Some(_err) = file::validate_filename(&name) {
            return;
        }
        let path = file::join_path(&self.current_path, &name);
        let (rt, api) = (self.rt.clone(), self.api.clone());
        cx.spawn(async move |this, cx| {
            let (api2, path2) = (api.clone(), path.clone());
            let result = api::call_api(&rt, move || async move { api2.create_folder(&path2).await }).await;
            let _ = this.update(cx, |this, cx| {
                if result.is_ok() {
                    let wh = this.window_handle;
                    let input = this.new_folder_input.clone();
                    let _ = wh.update(cx, |_, w, cx| {
                        input.update(cx, |s, cx| s.clean(w, cx));
                    });
                    this.load_folders(cx);
                }
            });
        }).detach();
    }

    fn confirm(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.is_blocked_selection(&self.current_path) {
            return;
        }
        let path = self.current_path.clone();
        (self.on_confirm)(path, window, cx);
        window.close_dialog(cx);
    }
}

impl Render for NetdiskPicker {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let current = self.current_path.clone();
        let blocked_current = self.is_blocked_selection(&current);

        let mut list = v_flex().gap_1().max_h(px(320.)).overflow_y_scrollbar();
        if self.loading {
            list = list.child(div().py_4().text_center().child("加载中..."));
        } else if self.folders.is_empty() {
            list = list.child(div().py_4().text_center().text_color(theme.muted_foreground).child("无子文件夹"));
        } else {
            for (ix, f) in self.folders.iter().enumerate() {
                let path = f.file.path.clone();
                let name = f.file.server_filename.clone();
                let disabled = self.is_blocked_entry(&path) || self.is_blocked_selection(&path);
                let enter_path = path.clone();
                list = list.child(
                    h_flex()
                        .id(("folder", ix))
                        .gap_2()
                        .items_center()
                        .px_2()
                        .py_1p5()
                        .rounded_md()
                        .when(!disabled, |this| this.cursor_pointer().hover(|s| s.bg(theme.secondary)))
                        .when(disabled, |this| this.opacity(0.4))
                        .child(Icon::new(IconName::Folder).size(px(18.)).text_color(theme.primary))
                        .child(div().flex_1().truncate().child(name))
                        .when(!disabled, |this| this.on_click(cx.listener(move |this, _, _, cx| {
                            this.navigate(enter_path.clone(), cx);
                        }))),
                );
            }
        }

        v_flex()
            .gap_3()
            .w(px(480.))
            .child(breadcrumb::render(&current, |target| {
                cx.listener(move |this, _, _, cx| this.navigate(target.clone(), cx))
            }))
            .child(
                h_flex().gap_2().items_center()
                    .child(Input::new(&self.new_folder_input).small())
                    .child(Button::new("np-mkdir").small().icon(IconName::FolderPlus).on_click(cx.listener(|this, _, _, cx| this.create_folder(cx)))),
            )
            .child(list)
            .child(
                h_flex().gap_2().justify_end()
                    .child(Button::new("np-cancel").label("取消").on_click(|_, w, cx| w.close_dialog(cx)))
                    .child(
                        Button::new("np-confirm").primary().label("选择此文件夹")
                            .disabled(blocked_current)
                            .on_click(cx.listener(|this, _, window, cx| this.confirm(window, cx))),
                    ),
            )
    }
}
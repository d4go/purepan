//! 转存对话框(移植 TransferDialog.vue 核心):输入分享链接/提取码/保存路径 →
//! 预览分享文件(可勾选)→ 创建转存任务。保持同步字段在 P5 接入。

use std::collections::HashSet;
use std::rc::Rc;
use std::sync::Arc;

use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::switch::Switch;
use gpui_kit::component::{ActiveTheme as _, Icon, Sizable as _, WindowExt as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AnyWindowHandle, App, AppContext as _, Context, Entity, InteractiveElement as _, IntoElement,
    ParentElement as _, Render, StatefulInteractiveElement as _, Styled as _, Window, div, px,
};
use serde_json::Value;

use crate::api::transfer::{CreateTransferRequest, SharedFileInfo};
use crate::api::{self, ApiClient};
use crate::bridge::ToastKind;
use crate::components::toast;
use crate::state::Backend;
use crate::util;

type SuccessFn = Rc<dyn Fn(&mut Window, &mut App)>;

pub struct TransferDialog {
    share_url: Entity<InputState>,
    password: Entity<InputState>,
    save_path: Entity<InputState>,
    auto_download: bool,
    files: Vec<SharedFileInfo>,
    selected: HashSet<u64>,
    share_info: Option<Value>,
    previewed: bool,
    loading: bool,
    creating: bool,
    rt: tokio::runtime::Handle,
    api: Arc<ApiClient>,
    on_success: SuccessFn,
    window_handle: AnyWindowHandle,
    /// 分享直下模式:目标为本地目录,save_path 由后端自动生成临时目录
    direct_mode: bool,
}

impl TransferDialog {
    pub fn new(direct_mode: bool, initial_path: &str, on_success: SuccessFn, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let b = cx.global::<Backend>();
        let (rt, api) = (b.rt.clone(), b.api.clone());
        let share_url = cx.new(|cx| InputState::new(window, cx).placeholder("分享链接,如 https://pan.baidu.com/s/xxxx"));
        let password = cx.new(|cx| InputState::new(window, cx).placeholder("提取码(可选,4 位)"));
        let save_path = cx.new(|cx| {
            let mut s = InputState::new(window, cx).placeholder("网盘保存路径");
            s.set_value(initial_path.to_string(), window, cx);
            s
        });
        Self {
            share_url, password, save_path,
            auto_download: false,
            files: Vec::new(), selected: HashSet::new(), share_info: None,
            previewed: false, loading: false, creating: false,
            rt, api, on_success, window_handle: window.window_handle(), direct_mode,
        }
    }

    fn toast(&self, cx: &mut Context<Self>, kind: ToastKind, msg: impl Into<String>) {
        let msg = msg.into();
        let wh = self.window_handle;
        let _ = wh.update(cx, |_, w, cx| toast::show(w, cx, kind, msg.clone()));
    }

    fn preview(&mut self, cx: &mut Context<Self>) {
        let url = self.share_url.read(cx).value().trim().to_string();
        if url.is_empty() { self.toast(cx, ToastKind::Warning, "请输入分享链接"); return; }
        let pwd = self.password.read(cx).value().trim().to_string();
        let pwd = if pwd.is_empty() { None } else { Some(pwd) };
        self.loading = true;
        cx.notify();
        let (rt, api) = (self.rt.clone(), self.api.clone());
        cx.spawn(async move |this, cx| {
            let (api2, url2, pwd2) = (api.clone(), url.clone(), pwd.clone());
            let result = api::call_api(&rt, move || async move {
                api2.preview_share_files(&url2, pwd2.as_deref(), 1, 100).await
            }).await;
            let _ = this.update(cx, |this, cx| {
                this.loading = false;
                match result {
                    Ok(resp) => {
                        this.files = resp.files;
                        this.share_info = resp.share_info;
                        this.selected = this.files.iter().map(|f| f.fs_id).collect();
                        this.previewed = true;
                    }
                    Err(api::ApiError::Business { code: 1001, .. }) => this.toast(cx, ToastKind::Warning, "该分享需要提取码,请输入"),
                    Err(api::ApiError::Business { code: 1002, .. }) => this.toast(cx, ToastKind::Error, "提取码错误"),
                    Err(e) => this.toast(cx, ToastKind::Error, e.message()),
                }
                cx.notify();
            });
        }).detach();
    }

    fn create(&mut self, cx: &mut Context<Self>) {
        let url = self.share_url.read(cx).value().trim().to_string();
        let pwd = self.password.read(cx).value().trim().to_string();
        let save_path = self.save_path.read(cx).value().trim().to_string();
        let selected_files: Vec<SharedFileInfo> = self.files.iter().filter(|f| self.selected.contains(&f.fs_id)).cloned().collect();
        let selected_fs_ids: Vec<u64> = selected_files.iter().map(|f| f.fs_id).collect();
        let req = CreateTransferRequest {
            share_url: url,
            password: if pwd.is_empty() { None } else { Some(pwd) },
            save_path: if self.direct_mode { String::new() } else { save_path },
            save_fs_id: 0,
            auto_download: Some(self.auto_download || self.direct_mode),
            local_download_path: None,
            is_share_direct_download: self.direct_mode,
            selected_fs_ids: Some(selected_fs_ids),
            selected_files: Some(selected_files),
            uid: None,
        };
        self.creating = true;
        cx.notify();
        let (rt, api) = (self.rt.clone(), self.api.clone());
        let on_success = self.on_success.clone();
        let wh = self.window_handle;
        cx.spawn(async move |this, cx| {
            let (api2, req2) = (api.clone(), req.clone());
            let result = api::call_api(&rt, move || async move { api2.create_transfer(&req2).await }).await;
            let _ = this.update(cx, |this, cx| {
                this.creating = false;
                match result {
                    Ok(resp) if !resp.need_password => {
                        this.toast(cx, ToastKind::Success, "转存任务已创建");
                        let _ = wh.update(cx, |_, w, cx| { (on_success)(w, cx); w.close_dialog(cx); });
                    }
                    Ok(_) => this.toast(cx, ToastKind::Warning, "需要提取码"),
                    Err(api::ApiError::Business { code: 1001, .. }) => this.toast(cx, ToastKind::Warning, "该分享需要提取码"),
                    Err(api::ApiError::Business { code: 1002, .. }) => this.toast(cx, ToastKind::Error, "提取码错误"),
                    Err(e) => this.toast(cx, ToastKind::Error, e.message()),
                }
                cx.notify();
            });
        }).detach();
    }

    fn pick_save_path(&mut self, cx: &mut Context<Self>) {
        let view = cx.entity().downgrade();
        let current = self.save_path.read(cx).value().to_string();
        let on_confirm: Rc<dyn Fn(String, &mut Window, &mut App)> = Rc::new(move |path, _w, cx| {
            let _ = view.update(cx, |this, cx| {
                let input = this.save_path.clone();
                let wh = this.window_handle;
                let _ = wh.update(cx, |_, w, cx| input.update(cx, |s, cx| s.set_value(path.clone(), w, cx)));
            });
        });
        let wh = self.window_handle;
        let _ = wh.update(cx, move |_, window, cx| {
            let picker = cx.new(|cx| crate::components::dialogs::netdisk_picker::NetdiskPicker::new(
                &current, Vec::new(), Vec::new(), on_confirm, window, cx,
            ));
            window.open_dialog(cx, move |d: gpui_kit::component::dialog::Dialog, _, _| d.title("选择保存路径").child(picker.clone()));
        });
    }
}

impl Render for TransferDialog {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let mut form = v_flex().gap_3().w(px(520.));
        form = form
            .child(h_flex().gap_2().items_center().child("分享链接").child(Input::new(&self.share_url)))
            .child(h_flex().gap_2().items_center().child("提取码").child(Input::new(&self.password)));
        if !self.direct_mode {
            form = form.child(h_flex().gap_2().items_center()
                .child("保存路径").child(Input::new(&self.save_path))
                .child(Button::new("td-pick").small().icon(IconName::Folder).on_click(cx.listener(|this, _, _, cx| this.pick_save_path(cx)))));
        }
        form = form.child(
            Switch::new("td-auto-dl").label(if self.direct_mode { "下载到本地(分享直下)" } else { "转存后自动下载" })
                .checked(self.auto_download || self.direct_mode)
                .on_change(cx.listener(|this, c, _, cx| { this.auto_download = *c; cx.notify(); })),
        );

        if self.previewed {
            let mut list = v_flex().max_h(px(260.)).overflow_y_scrollbar().border_1().border_color(theme.border).rounded_md();
            let picker = cx.entity().downgrade();
            for (ix, f) in self.files.iter().enumerate() {
                let fs_id = f.fs_id;
                let checked = self.selected.contains(&fs_id);
                let picker = picker.clone();
                list = list.child(
                    h_flex().gap_2().items_center().px_2().py_1()
                        .child(Checkbox::new(("td-file", ix)).checked(checked).on_change(move |c, _, cx| {
                            let c = *c;
                            let _ = picker.update(cx, |this, cx| {
                                if c { this.selected.insert(fs_id); } else { this.selected.remove(&fs_id); }
                                cx.notify();
                            });
                        }))
                        .child(Icon::new(if f.is_dir { IconName::Folder } else { IconName::FileText }).size(px(16.)))
                        .child(div().flex_1().truncate().child(f.name.clone()))
                        .child(div().text_color(theme.muted_foreground).child(if f.is_dir { "-".to_string() } else { util::format_size(f.size) })),
                );
            }
            form = form.child(list);
        }

        let previewed = self.previewed;
        let loading = self.loading;
        let creating = self.creating;
        form = form.child(
            h_flex().gap_2().justify_end()
                .child(Button::new("td-cancel").label("取消").on_click(|_, w, cx| w.close_dialog(cx)))
                .when(!previewed, |this| this.child(
                    Button::new("td-preview").primary().loading(loading).label("预览文件")
                        .on_click(cx.listener(|this, _, _, cx| this.preview(cx)))))
                .when(previewed, |this| this.child(
                    Button::new("td-create").primary().loading(creating).label(if self.direct_mode { "开始直下" } else { "开始转存" })
                        .on_click(cx.listener(|this, _, _, cx| this.create(cx))))),
        );
        form
    }
}
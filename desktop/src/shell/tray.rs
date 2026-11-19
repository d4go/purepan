//! 系统托盘(Linux,ksni/StatusNotifierItem)。菜单:显示主窗口 / 退出程序;左键显示窗口。

use ksni::blocking::TrayMethods;
use ksni::menu::StandardItem;
use ksni::{MenuItem, Tray};

use crate::bridge::UiEvent;

pub struct PurepanTray {
    ui: async_channel::Sender<UiEvent>,
}

impl Tray for PurepanTray {
    fn id(&self) -> String {
        "purepan".into()
    }

    fn title(&self) -> String {
        "PurePan".into()
    }

    fn icon_pixmap(&self) -> Vec<ksni::Icon> {
        vec![load_icon()]
    }

    fn activate(&mut self, _x: i32, _y: i32) {
        let _ = self.ui.try_send(UiEvent::ActivateWindow);
    }

    fn menu(&self) -> Vec<MenuItem<Self>> {
        vec![
            MenuItem::Standard(StandardItem {
                label: "显示主窗口".into(),
                activate: Box::new(|t: &mut Self| {
                    let _ = t.ui.try_send(UiEvent::ActivateWindow);
                }),
                ..Default::default()
            }),
            MenuItem::Standard(StandardItem {
                label: "退出程序".into(),
                activate: Box::new(|t: &mut Self| {
                    let _ = t.ui.try_send(UiEvent::Quit);
                }),
                ..Default::default()
            }),
        ]
    }
}

fn load_icon() -> ksni::Icon {
    let bytes = include_bytes!("../../assets/icons/icon.png");
    match image::load_from_memory(bytes) {
        Ok(img) => {
            let rgba = img.to_rgba8();
            let (w, h) = rgba.dimensions();
            let mut data = Vec::with_capacity((w * h * 4) as usize);
            for p in rgba.pixels() {
                let [r, g, b, a] = p.0;
                data.extend_from_slice(&[a, r, g, b]); // ARGB32 network order
            }
            ksni::Icon { width: w as i32, height: h as i32, data }
        }
        Err(_) => ksni::Icon { width: 1, height: 1, data: vec![0, 0, 0, 0] },
    }
}

/// 启动托盘(后台线程)。失败(无 DBus/SNI 宿主)时静默降级。
pub fn spawn(ui: async_channel::Sender<UiEvent>) {
    let tray = PurepanTray { ui };
    std::thread::Builder::new()
        .name("purepan-tray".into())
        .spawn(move || {
            if let Err(e) = tray.spawn() {
                tracing::warn!("托盘启动失败(可能无 SNI 宿主): {e}");
            }
        })
        .ok();
}
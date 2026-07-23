#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use purepan_backend::start_server;
use tauri::menu::{MenuBuilder, MenuItemBuilder};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::Manager;

fn configure_runtime_directory(app: &tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let runtime_dir = app.path().app_data_dir()?;
    std::fs::create_dir_all(&runtime_dir)?;
    std::env::set_current_dir(&runtime_dir)?;
    eprintln!("PurePan runtime data directory: {}", runtime_dir.display());
    Ok(())
}

fn show_main_window(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

/// 主窗口完成首帧后再显示，避免用户看到 WebView 的白屏阶段。
#[tauri::command]
fn frontend_ready(app: tauri::AppHandle) -> Result<(), String> {
    let main = app
        .get_webview_window("main")
        .ok_or_else(|| "main window not found".to_string())?;

    main.show().map_err(|error| error.to_string())?;
    main.set_focus().map_err(|error| error.to_string())?;

    Ok(())
}

fn main() {
    tauri::Builder::default()
        // This plugin must be registered first so a repeated launch exits before
        // any other application services or windows are initialized.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            show_main_window(app);
        }))
        .plugin(tauri_plugin_shell::init())
        .invoke_handler(tauri::generate_handler![frontend_ready])
        .setup(|app| {
            if !cfg!(debug_assertions) {
                configure_runtime_directory(app)?;
            }

            // 在 Tauri 的 tokio 运行时中启动内嵌 Axum 服务器
            tauri::async_runtime::spawn(async move {
                if let Err(e) = start_server().await {
                    eprintln!("内嵌服务器错误: {}", e);
                }
            });

            // 读取配置
            // 创建托盘菜单
            let show_item = MenuItemBuilder::new("显示主窗口").id("show").build(app)?;
            let quit_item = MenuItemBuilder::new("退出程序").id("quit").build(app)?;
            let menu = MenuBuilder::new(app)
                .item(&show_item)
                .item(&quit_item)
                .build()?;

            // 创建托盘图标
            let _tray = TrayIconBuilder::new()
                .icon(
                    app.default_window_icon()
                        .cloned()
                        .expect("默认窗口图标不存在"),
                )
                .menu(&menu)
                .show_menu_on_left_click(false)
                .tooltip("PurePan")
                .on_menu_event(move |app, event| match event.id.as_ref() {
                    "show" => {
                        show_main_window(app);
                    }
                    "quit" => {
                        app.exit(0);
                    }
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        show_main_window(tray.app_handle());
                    }
                })
                .build(app)?;

            // 监听窗口关闭事件
            if let Some(window) = app.get_webview_window("main") {
                let app_handle = app.handle().clone();
                window.on_window_event(move |event| {
                    if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                        // Re-read the setting for every close request so a change
                        // saved from Settings takes effect without an app restart.
                        if read_close_behavior() == "minimize_to_tray" {
                            api.prevent_close();
                            if let Some(window) = app_handle.get_webview_window("main") {
                                let _ = window.hide();
                            }
                        }
                    }
                });
            }

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

fn read_close_behavior() -> String {
    let config_path = "config/app.toml";
    if let Ok(content) = std::fs::read_to_string(config_path) {
        if let Ok(value) = content.parse::<toml::Value>() {
            if let Some(behavior) = value
                .get("ui")
                .and_then(|ui| ui.get("close_behavior"))
                .and_then(|v| v.as_str())
            {
                return behavior.to_string();
            }
        }
    }
    "close".to_string()
}

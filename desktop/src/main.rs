#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod api;
mod app;
mod backend_embed;
mod bridge;
mod components;
mod shell;
mod state;
mod theme;
mod util;
mod views;
mod ws;

use gpui_kit::AppContext as _;
use state::{AuthStore, BackendHealth, TaskCache, WebAuthState};

fn main() {
    // 日志由内嵌后端统一初始化(tracing-appender → logs/),桌面端不再单独设置
    // 全局 subscriber,避免与后端 init_logging 冲突。

    // 后端所有数据路径都相对 CWD,必须先切换数据目录再启动内嵌服务器
    let data_dir = shell::data_dir::init();
    eprintln!("PurePan 数据目录: {}", data_dir.display());

    let (ui_tx, ui_rx) = async_channel::bounded::<bridge::UiEvent>(256);

    // 单实例:二次启动通知主实例聚焦后退出
    #[cfg(unix)]
    if !shell::single_instance::ensure_single_instance(ui_tx.clone()) {
        return;
    }

    // 系统托盘(Linux)
    #[cfg(target_os = "linux")]
    shell::tray::spawn(ui_tx.clone());

    let backend = backend_embed::spawn(ui_tx);

    gpui_kit::application()
        .with_assets(gpui_kit::assets::AllAssets)
        .run(move |cx| {
            gpui_kit::init(cx);
            theme::init(cx);
            cx.set_global(backend);
            cx.set_global(AuthStore::default());
            cx.set_global(WebAuthState::default());
            cx.set_global(BackendHealth::default());
            cx.set_global(TaskCache::default());

            let controller = cx.new(|cx| app::AppController::new(cx, ui_rx));
            app::open_initial(cx, controller);
        });
}
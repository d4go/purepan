pub mod data_dir;
pub mod window;

#[cfg(target_os = "linux")]
pub mod tray;

#[cfg(unix)]
pub mod single_instance;
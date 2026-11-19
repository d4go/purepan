//! 单实例(Unix socket):二次启动时通知主实例聚焦窗口后退出。

use std::io::{Read, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;

use crate::bridge::UiEvent;

fn socket_path() -> PathBuf {
    std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join("ipc.sock")
}

/// 返回 true 表示主实例;false 表示已有实例运行(已通知其聚焦,调用方应退出)。
pub fn ensure_single_instance(ui: async_channel::Sender<UiEvent>) -> bool {
    let path = socket_path();
    // 已有实例?尝试连接并发送 show
    if let Ok(mut stream) = UnixStream::connect(&path) {
        let _ = stream.write_all(b"show");
        return false;
    }
    // 清理陈旧 socket 后绑定
    let _ = std::fs::remove_file(&path);
    let listener = match UnixListener::bind(&path) {
        Ok(l) => l,
        Err(_) => return true, // 绑定失败不阻塞启动
    };
    std::thread::Builder::new()
        .name("purepan-ipc".into())
        .spawn(move || {
            for stream in listener.incoming() {
                if let Ok(mut s) = stream {
                    let mut buf = [0u8; 16];
                    if let Ok(n) = s.read(&mut buf) {
                        if &buf[..n] == b"show" {
                            let _ = ui.try_send(UiEvent::ActivateWindow);
                        }
                    }
                }
            }
        })
        .ok();
    true
}
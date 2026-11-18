use std::path::PathBuf;

/// 初始化数据目录并切换 CWD(后端所有数据路径都相对 CWD)。
///
/// - `PUREPAN_DATA_DIR` 环境变量优先(测试/便携模式)
/// - debug:crate 目录(同 Tauri dev 的 src-tauri,自带 config/app.toml)
/// - release:`~/.local/share/com.d4go.PurePan`(与 Tauri 一致,保留老用户数据)
pub fn init() -> PathBuf {
    let dir = if let Ok(d) = std::env::var("PUREPAN_DATA_DIR") {
        PathBuf::from(d)
    } else if cfg!(debug_assertions) {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
    } else {
        dirs::data_dir()
            .expect("无法确定用户数据目录")
            .join("com.d4go.PurePan")
    };
    std::fs::create_dir_all(&dir).expect("无法创建数据目录");
    std::env::set_current_dir(&dir).expect("无法切换工作目录");
    dir
}
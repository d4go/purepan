use gpui_kit::Global;

/// 后端就绪状态(启动引导维护)。
#[derive(Default)]
pub struct BackendHealth {
    pub connected: bool,
}

impl Global for BackendHealth {}
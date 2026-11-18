use gpui_kit::Global;

/// 后端连通状态(由健康检查轮询维护)。
#[derive(Default)]
pub struct BackendHealth {
    pub connected: bool,
}

impl Global for BackendHealth {}
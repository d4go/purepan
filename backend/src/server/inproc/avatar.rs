//! 头像等外部图片的进程内下载(复用后端代理配置)。

use std::time::Duration;

use super::InprocError;
use crate::server::AppState;

const MAX_IMAGE_BYTES: usize = 4 * 1024 * 1024;

pub async fn fetch(state: &AppState, url: String) -> Result<Vec<u8>, InprocError> {
    if !(url.starts_with("https://") || url.starts_with("http://")) {
        return Err(InprocError::new(400, "图片地址无效"));
    }

    let proxy = {
        let cfg = state.config.read().await;
        cfg.network.proxy.clone()
    };

    let mut builder = reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .user_agent("PurePan/0.1.5");
    if let Ok(Some(proxy)) = proxy.to_reqwest_proxy() {
        builder = builder.proxy(proxy);
    }
    let client = builder
        .build()
        .map_err(|e| InprocError::new(-1, format!("图片客户端创建失败: {e}")))?;

    let resp = client
        .get(&url)
        .send()
        .await
        .map_err(|e| InprocError::new(-1, format!("图片加载失败: {e}")))?;
    if !resp.status().is_success() {
        return Err(InprocError::new(
            resp.status().as_u16() as i64,
            "图片加载失败",
        ));
    }
    if resp
        .content_length()
        .is_some_and(|len| len as usize > MAX_IMAGE_BYTES)
    {
        return Err(InprocError::new(-1, "图片过大"));
    }
    let bytes = resp
        .bytes()
        .await
        .map_err(|e| InprocError::new(-1, format!("图片读取失败: {e}")))?;
    if bytes.len() > MAX_IMAGE_BYTES {
        return Err(InprocError::new(-1, "图片过大"));
    }
    Ok(bytes.to_vec())
}

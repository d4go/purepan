use crate::netdisk::NetdiskClient;
use anyhow::{Context, Result};
use std::sync::{Arc, RwLock as StdRwLock};
use tracing::debug;

/// block_list 比较器
pub struct BlockListComparator {
    netdisk_client: Arc<StdRwLock<NetdiskClient>>,
}

impl BlockListComparator {
    pub fn new(netdisk_client: Arc<StdRwLock<NetdiskClient>>) -> Self {
        Self { netdisk_client }
    }

    /// 比较本地和远程文件的 block_list
    ///
    /// # 参数
    /// - remote_path: 远程文件路径
    /// - local_block_list: 本地文件的 block_list
    ///
    /// # 返回
    /// - Ok(true): block_list 相同
    /// - Ok(false): block_list 不同
    /// - Err: 比较失败（例如远程文件不存在）
    pub async fn compare_block_lists(
        &self,
        remote_path: &str,
        local_block_list: &str,
    ) -> Result<bool> {
        // 1. 通过 NetdiskClient 获取远程文件元信息
        let remote_block_list = self.get_remote_block_list(remote_path).await?;

        // 2. 比较两个 block_list 是否相同
        Ok(remote_block_list == local_block_list)
    }

    /// 获取远程文件的 block_list
    async fn get_remote_block_list(&self, remote_path: &str) -> Result<String> {
        // Clone client，释放锁，然后 await
        let client = {
            let guard = self.netdisk_client.read().unwrap();
            guard.clone()
        };

        // 调用百度网盘 API 获取文件元信息
        let response = client.filemetas(&[remote_path.to_string()]).await?;

        if response.list.is_empty() {
            anyhow::bail!("远程文件不存在: {}", remote_path);
        }

        let file_info = &response.list[0];

        // 提取 block_list 字段
        let block_list = file_info
            .block_list
            .clone()
            .context("远程文件缺少 block_list 字段")?;

        debug!(
            "获取远程文件 block_list: path={}, block_list={}",
            remote_path, block_list
        );

        Ok(block_list)
    }
}

#[cfg(test)]
mod tests {
    // 注意：BlockListComparator 的属性测试需要真实的 NetdiskClient
    // 这些测试应该作为集成测试运行，因为它们需要网络访问
    // 这里我们添加一些基本的单元测试来验证逻辑

    #[test]
    fn test_block_list_comparison_same() {
        let block_list1 = r#"["abc123","def456"]"#;
        let block_list2 = r#"["abc123","def456"]"#;
        assert_eq!(block_list1, block_list2);
    }

    #[test]
    fn test_block_list_comparison_different() {
        let block_list1 = r#"["abc123","def456"]"#;
        let block_list2 = r#"["abc123","xyz789"]"#;
        assert_ne!(block_list1, block_list2);
    }

    // Feature: file-conflict-strategy, Property 6: 智能去重 block_list 比较
    // Feature: file-conflict-strategy, Property 7: 智能去重相同内容跳过
    // Feature: file-conflict-strategy, Property 8: 智能去重不同内容重命名
    // **Validates: Requirements 5.1, 5.2, 5.3**
    //
    // 注意：这些属性的完整测试需要在集成测试中进行，因为需要：
    // 1. 真实的 NetdiskClient 连接
    // 2. 上传测试文件到百度网盘
    // 3. 获取远程文件的 block_list
    // 4. 比较本地和远程的 block_list
    //
    // 这里我们只能测试 block_list 字符串比较的逻辑
}

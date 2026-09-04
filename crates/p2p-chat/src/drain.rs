//! CLI one-shot 排空（D5）：Chat::drain_peer 实现落点。
//! 原 lib.rs 内联，行数红线（300）机械下沉至此，签名与语义零变更。

use std::time::Duration;

use crate::model::{parse_peer_id, ChatError};
use crate::Chat;

impl Chat {
    /// 排空该 peer 的 outbox（CLI one-shot，D5）：返回补投条目数。
    pub async fn drain_peer(&self, peer: &str, budget: Duration) -> Result<usize, ChatError> {
        let pid = parse_peer_id(peer)?;
        let before = self.core.store.outbox_for(peer).len();
        if before == 0 {
            return Ok(0);
        }
        self.core
            .node
            .connect(pid)
            .await
            .map_err(|e| ChatError::ConnectFailed(format!("连接 {peer} 失败：{e}")))?;
        let _ = tokio::time::timeout(budget, crate::outbox::flush_peer(&self.core, peer)).await;
        let after = self.core.store.outbox_for(peer).len();
        Ok(before.saturating_sub(after))
    }
}

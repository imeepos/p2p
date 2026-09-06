//! 线协议 /im/ginvite/1 客户端（wire-protocol.md §8.4 登记）。
//! 帧纪律与 /im/invite/1 完全同构：载荷首字节 = 类型头，其余为 JSON；
//! 每流一请求一 ACK；peer 字段 = 发端自身 PeerId。
//! 入站 handler 在 ginvite_handler.rs，门面在 ginvite_api.rs（行数红线拆分）。

use p2p_identity::PeerId;
use p2p_protocol::ProtocolId;
use tokio::io::AsyncWriteExt;

use crate::ginvite::GroupInvite;
use crate::model::{parse_peer_id, ChatError};
use crate::wire::{read_ack, write_typed, AckFrame};
use crate::ChatCore;

pub(crate) const GINVITE: u8 = 0x01;
pub(crate) const GACCEPT: u8 = 0x02;
pub(crate) const GREJECT: u8 = 0x03;

/// 线协议 ID（wire-protocol.md §8.4 登记，与 /im/invite/1 并存路由）。
pub const GINVITE_PROTOCOL: &str = "/im/ginvite/1";

/// 线上动作帧：peer = 发端自身 PeerId；reason 仅 GREJECT 语义携带；
/// note? 备注随 GINVITE；同一结构承载三类动作（与 /im/invite/1 同构口径）。
#[derive(serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GinviteFrame {
    pub(crate) id: String,
    pub(crate) peer: String,
    pub(crate) group_id: String,
    pub(crate) group_name: String,
    pub(crate) owner: String,
    pub(crate) inviter_nickname: String,
    pub(crate) note: Option<String>,
    pub(crate) reason: Option<String>,
}

impl GinviteFrame {
    /// inviter_nickname 由调用方解析（卡片历史优先，缺省回退 PeerId 缩略）：
    /// 邀请条目字段冻结，不承载昵称（契约 §8.4）。
    pub(crate) fn new(
        local: &PeerId,
        invite: &GroupInvite,
        inviter_nickname: &str,
        reason: Option<String>,
    ) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            peer: local.to_string(),
            group_id: invite.group_id.clone(),
            group_name: invite.group_name.clone(),
            owner: invite.owner.clone(),
            inviter_nickname: inviter_nickname.to_string(),
            note: invite.note.clone(),
            reason,
        }
    }
}

pub(crate) fn ack_for(frame: &GinviteFrame) -> AckFrame {
    AckFrame {
        id: frame.id.clone(),
        ok: true,
        reason: None,
    }
}

/// 客户端发送：连接 → 开流 → 写帧 → 读 ACK（持 peer 串行锁，与聊天投递互斥）。
/// delivered 标记由调用方按方向语义落账（out=邀请送达 / in=决策回送）。
pub(crate) async fn deliver_frame(
    core: &ChatCore,
    peer: &str,
    kind: u8,
    frame: &GinviteFrame,
) -> Result<(), ChatError> {
    let _guard = core.peer_guard(peer).await;
    let pid = parse_peer_id(peer)?;
    core.node
        .connect(pid)
        .await
        .map_err(|e| ChatError::ConnectFailed(format!("连接 {peer} 失败：{e}")))?;
    let proto =
        ProtocolId::new(GINVITE_PROTOCOL).map_err(|e| ChatError::Protocol(e.to_string()))?;
    let mut stream = core
        .node
        .new_stream(pid, proto)
        .await
        .map_err(|e| ChatError::StreamFailed(format!("开流失败：{e}")))?;
    let bytes = serde_json::to_vec(frame).map_err(ChatError::Json)?;
    write_typed(&mut stream, kind, &bytes).await?;
    stream.flush().await?;
    let ack = tokio::time::timeout(crate::core::ACK_TIMEOUT, read_ack(&mut stream))
        .await
        .map_err(|_| ChatError::StreamFailed("等待入群邀请 ACK 超时".into()))??;
    if ack.id != frame.id {
        return Err(ChatError::Protocol(format!(
            "入群邀请 ACK id 不匹配：{} ≠ {}",
            ack.id, frame.id
        )));
    }
    if !ack.ok {
        return Err(ChatError::SendFailed(format!(
            "对端拒绝入群邀请帧：{}",
            ack.reason.as_deref().unwrap_or("")
        )));
    }
    Ok(())
}

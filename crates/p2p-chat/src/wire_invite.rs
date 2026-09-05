//! 线协议 /im/invite/1（邀请制加好友，wire-protocol.md §8.3 登记）。
//! 帧载荷首字节 = 类型头：INVITE 0x01 / ACCEPT 0x02 / REJECT 0x03 / ACK 0x04，
//! 其余为 JSON；每流一请求一 ACK（与 /im/chat/1 同纪律，peer 字段 = 发端自身 PeerId）。
//!
//! 自愈语义：收到 INVITE 时若已是好友或存在本机对对方的待同意邀请（互邀），
//! 视为同意——本机先建好友，再回投 ACCEPT；对方离线时由其重连重投 INVITE 收敛。

use p2p_identity::PeerId;
use p2p_mux::BoxedStream;
use p2p_protocol::ProtocolId;
use tokio::io::AsyncWriteExt;

use crate::invite::InviteDirection;
use crate::model::{parse_peer_id, ChatError};
use crate::wire::{write_typed, AckFrame};
use crate::ChatCore;

pub(crate) const INVITE: u8 = 0x01;
pub(crate) const ACCEPT: u8 = 0x02;
pub(crate) const REJECT: u8 = 0x03;

/// 线上邀请帧：peer = 发端自身 PeerId；addrs = 发端可回拨地址（INVITE）。
#[derive(serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct InviteFrame {
    pub(crate) id: String,
    pub(crate) peer: String,
    pub(crate) nickname: String,
    pub(crate) addrs: Vec<String>,
}

impl InviteFrame {
    pub(crate) fn new(local: &PeerId, nickname: &str, addrs: Vec<String>) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            peer: local.to_string(),
            nickname: nickname.to_string(),
            addrs,
        }
    }
}

pub(crate) fn ack_for(frame: &InviteFrame) -> AckFrame {
    AckFrame {
        id: frame.id.clone(),
        ok: true,
        reason: None,
    }
}

/// 客户端发送：连接 → 开流 → 写帧 → 读 ACK（持 peer 串行锁，与聊天投递互斥）。
/// only_undelivered（仅 INVITE）：锁内复查 delivered 标记，已送达即跳过——
/// 消除 PeerConnected 重投与用户操作（同意/拒绝）之间的重复投递竞态。
pub(crate) async fn deliver_frame(
    core: &ChatCore,
    peer: &str,
    kind: u8,
    frame: &InviteFrame,
    only_undelivered: bool,
) -> Result<(), ChatError> {
    let _guard = core.peer_guard(peer).await;
    if only_undelivered && kind == INVITE {
        let fresh = core
            .store
            .invites_list()?
            .into_iter()
            .any(|i| i.peer_id == peer && i.direction == InviteDirection::Out && i.delivered);
        if fresh {
            tracing::debug!(peer = %peer, "邀请已送达过，重投跳过");
            return Ok(());
        }
    }
    let pid = parse_peer_id(peer)?;
    core.node
        .connect(pid)
        .await
        .map_err(|e| ChatError::ConnectFailed(format!("连接 {peer} 失败：{e}")))?;
    let proto =
        ProtocolId::new(crate::INVITE_PROTOCOL).map_err(|e| ChatError::Protocol(e.to_string()))?;
    let mut stream = core
        .node
        .new_stream(pid, proto)
        .await
        .map_err(|e| ChatError::StreamFailed(format!("开流失败：{e}")))?;
    let bytes = serde_json::to_vec(frame).map_err(ChatError::Json)?;
    write_typed(&mut stream, kind, &bytes).await?;
    stream.flush().await?;
    let ack = tokio::time::timeout(crate::core::ACK_TIMEOUT, read_invite_ack(&mut stream))
        .await
        .map_err(|_| ChatError::StreamFailed("等待邀请 ACK 超时".into()))??;
    if ack.id != frame.id {
        return Err(ChatError::Protocol(format!(
            "邀请 ACK id 不匹配：{} ≠ {}",
            ack.id, frame.id
        )));
    }
    if !ack.ok {
        return Err(ChatError::SendFailed(format!(
            "对端拒绝邀请帧：{}",
            ack.reason.as_deref().unwrap_or("")
        )));
    }
    if kind == INVITE {
        core.store
            .mark_invite_delivered(peer, InviteDirection::Out)?;
    }
    Ok(())
}

async fn read_invite_ack(r: &mut BoxedStream) -> Result<AckFrame, ChatError> {
    crate::wire::read_ack(r)
        .await
        .map_err(|e| ChatError::StreamFailed(format!("读 ACK 失败：{e}")))
}

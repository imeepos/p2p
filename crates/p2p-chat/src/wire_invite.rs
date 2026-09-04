//! 线协议 /im/invite/1（邀请制加好友，wire-protocol.md §8.2 登记）。
//! 帧载荷首字节 = 类型头：INVITE 0x01 / ACCEPT 0x02 / REJECT 0x03 / ACK 0x04，
//! 其余为 JSON；每流一请求一 ACK（与 /im/chat/1 同纪律，peer 字段 = 发端自身 PeerId）。
//!
//! 自愈语义：收到 INVITE 时若已是好友或存在本机对对方的待同意邀请（互邀），
//! 视为同意——本机先建好友，再回投 ACCEPT；对方离线时由其重连重投 INVITE 收敛。

use std::io;
use std::sync::Arc;

use async_trait::async_trait;
use p2p::ProtocolHandler;
use p2p_identity::PeerId;
use p2p_mux::BoxedStream;
use p2p_protocol::{read_frame, ProtocolId};
use tokio::io::AsyncWriteExt;

use crate::invite::{FriendInvite, InviteDirection, InviteState};
use crate::events::ChatEvent;
use crate::model::{now_ms, parse_peer_id, validate_nickname, ChatError};
use crate::wire::{write_typed, AckFrame, ACK};
use crate::{invite_api, ChatCore};

pub(crate) const INVITE: u8 = 0x01;
pub(crate) const ACCEPT: u8 = 0x02;
pub(crate) const REJECT: u8 = 0x03;

/// 线上邀请帧：peer = 发端自身 PeerId；addrs = 发端可回拨地址（INVITE）。
#[derive(serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct InviteFrame {
    id: String,
    peer: String,
    nickname: String,
    addrs: Vec<String>,
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

fn ack_for(frame: &InviteFrame) -> AckFrame {
    AckFrame {
        id: frame.id.clone(),
        ok: true,
        reason: None,
    }
}

/// 入站 /im/invite/1 handler。
pub(crate) struct InviteHandler {
    core: Arc<ChatCore>,
    proto: ProtocolId,
}

impl InviteHandler {
    pub(crate) fn new(core: Arc<ChatCore>, proto: ProtocolId) -> Self {
        Self { core, proto }
    }
}

#[async_trait]
impl ProtocolHandler for InviteHandler {
    fn protocol(&self) -> ProtocolId {
        self.proto.clone()
    }

    async fn handle(&self, mut stream: BoxedStream) -> io::Result<()> {
        let outcome = self.handle_inbound(&mut stream).await;
        if let Err(e) = &outcome {
            tracing::warn!(error = %e, "/im/invite/1 入站帧校验失败，断流");
        }
        outcome
    }
}

impl InviteHandler {
    async fn handle_inbound(&self, stream: &mut BoxedStream) -> io::Result<()> {
        let frame = read_frame(stream).await?;
        let Some((&kind, payload)) = frame.split_first() else {
            return Err(io::Error::other("邀请帧缺类型头"));
        };
        let parsed: InviteFrame = serde_json::from_slice(payload)
            .map_err(|e| io::Error::other(format!("邀请帧 JSON 非法：{e}")))?;
        let local = self.core.node.local_peer_id();
        let peer = validate_sender(&parsed, local)?;
        let core = self.core.clone();
        let outcome = match kind {
            INVITE => on_invite(&core, &parsed, peer).await,
            ACCEPT => on_accept(&core, &parsed, peer).await,
            REJECT => on_reject(&core, &parsed, peer).await,
            other => {
                return Err(io::Error::other(format!(
                    "未知邀请帧类型 {other:#04x}，断流"
                )));
            }
        };
        if let Err(e) = outcome {
            tracing::warn!(peer = %parsed.peer, error = %e, "邀请帧处理失败，回 ACK nack");
            let nack = AckFrame {
                id: parsed.id.clone(),
                ok: false,
                reason: Some(e.to_string()),
            };
            return write_ack(stream, &nack).await;
        }
        write_ack(stream, &ack_for(&parsed)).await
    }
}

async fn write_ack(stream: &mut BoxedStream, ack: &AckFrame) -> io::Result<()> {
    let bytes = serde_json::to_vec(ack).map_err(io::Error::other)?;
    write_typed(stream, ACK, &bytes).await?;
    stream.flush().await
}

/// 发端校验：peer 合法、非本机（同 /im/chat/1 纵深防御口径）。
fn validate_sender(frame: &InviteFrame, local: PeerId) -> Result<PeerId, io::Error> {
    let peer = parse_peer_id(&frame.peer).map_err(|e| io::Error::other(e.to_string()))?;
    if peer == local {
        return Err(io::Error::other("入站邀请帧 peer 指向本机，疑似伪装"));
    }
    Ok(peer)
}

/// INVITE：已是好友/互邀 → 本机建好友并回投 ACCEPT（自愈收敛）；否则登记来邀。
async fn on_invite(core: &Arc<ChatCore>, frame: &InviteFrame, peer: PeerId) -> Result<(), ChatError> {
    let peer_s = peer.to_string();
    let friends = core.store.friends_list()?;
    if friends.iter().any(|f| f.peer_id == peer_s) {
        spawn_reply(core, peer_s, ACCEPT, "", Vec::new());
        return Ok(());
    }
    let out = core
        .store
        .invites_list()?
        .into_iter()
        .find(|i| i.peer_id == peer_s && i.direction == InviteDirection::Out);
    let nickname = display_name(&frame.nickname, &peer_s);
    match out {
        Some(pending) => {
            invite_api::insert_friend(core, &peer_s, &pending.nickname, pending.addrs.clone(), pending.note.clone())?;
            core.store.remove_invite(&peer_s, InviteDirection::Out)?;
            emit(core, &peer_s, InviteState::Accepted);
            spawn_reply(core, peer_s, ACCEPT, "", Vec::new());
        }
        None => {
            core.store.upsert_invite(FriendInvite {
                peer_id: peer_s.clone(),
                nickname,
                addrs: frame.addrs.clone(),
                note: None,
                direction: InviteDirection::In,
                ts_ms: now_ms(),
                delivered: true,
            })?;
            emit(core, &peer_s, InviteState::Incoming);
        }
    }
    Ok(())
}

/// ACCEPT：凭本机待发邀请建好友（双向完成）；无邀请时采信对端自称（其已同意，留告警）。
async fn on_accept(core: &Arc<ChatCore>, frame: &InviteFrame, peer: PeerId) -> Result<(), ChatError> {
    let peer_s = peer.to_string();
    let friends = core.store.friends_list()?;
    if friends.iter().any(|f| f.peer_id == peer_s) {
        let _ = core.store.remove_invite(&peer_s, InviteDirection::Out);
        return Ok(());
    }
    match core
        .store
        .invites_list()?
        .into_iter()
        .find(|i| i.peer_id == peer_s && i.direction == InviteDirection::Out)
    {
        Some(pending) => {
            invite_api::insert_friend(core, &peer_s, &pending.nickname, pending.addrs.clone(), pending.note.clone())?;
            core.store.remove_invite(&peer_s, InviteDirection::Out)?;
        }
        None => {
            tracing::warn!(peer = %peer_s, "收到无待发邀请的 ACCEPT，采信对端同意建好友");
            let nickname = display_name(&frame.nickname, &peer_s);
            invite_api::insert_friend(core, &peer_s, &nickname, frame.addrs.clone(), None)?;
        }
    }
    emit(core, &peer_s, InviteState::Accepted);
    Ok(())
}

/// REJECT：移除本机待发邀请并发 rejected 事件（无邀请也幂等成功）。
async fn on_reject(core: &Arc<ChatCore>, _frame: &InviteFrame, peer: PeerId) -> Result<(), ChatError> {
    let peer_s = peer.to_string();
    let removed = core.store.remove_invite(&peer_s, InviteDirection::Out)?;
    if !removed {
        tracing::warn!(peer = %peer_s, "收到无待发邀请的 REJECT（幂等处理）");
    }
    emit(core, &peer_s, InviteState::Rejected);
    Ok(())
}

fn emit(core: &ChatCore, peer: &str, state: InviteState) {
    let _ = core.events.send(ChatEvent::ChatInvite {
        peer: peer.to_string(),
        state,
    });
}

/// 空昵称回退 peer 全串（GUI 展示层可再缩略）。
fn display_name(raw: &str, peer: &str) -> String {
    match validate_nickname(raw) {
        Ok(n) if !n.is_empty() => n,
        _ => peer.to_string(),
    }
}

/// 回投帧（自愈/同意路径）：fire-and-forget，失败留告警由重连重投收敛。
fn spawn_reply(core: &Arc<ChatCore>, peer: String, kind: u8, nickname: &str, addrs: Vec<String>) {
    let core = core.clone();
    let nickname = nickname.to_string();
    tokio::spawn(async move {
        let local = core.node.local_peer_id();
        let frame = InviteFrame::new(&local, &nickname, addrs);
        if let Err(e) = deliver_frame(&core, &peer, kind, &frame, false).await {
            tracing::warn!(peer = %peer, kind, error = %e, "邀请回投失败，等待对端重连收敛");
        }
    });
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
    let proto = ProtocolId::new(crate::INVITE_PROTOCOL)
        .map_err(|e| ChatError::Protocol(e.to_string()))?;
    let mut stream = core
        .node
        .new_stream(pid, proto)
        .await
        .map_err(|e| ChatError::StreamFailed(format!("开流失败：{e}")))?;
    let bytes = serde_json::to_vec(frame).map_err(ChatError::Json)?;
    write_typed(&mut stream, kind, &bytes).await?;
    stream.flush().await?;
    let ack = tokio::time::timeout(crate::core::ack_timeout(), read_invite_ack(&mut stream))
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

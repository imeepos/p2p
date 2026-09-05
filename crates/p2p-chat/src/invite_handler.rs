//! /im/invite/1 入站 handler 与语义分支（INVITE/ACCEPT/REJECT 处理、自愈回投）。
//! 协议帧与客户端投递在 wire_invite.rs（行数红线拆分）。

use std::io;
use std::sync::Arc;

use async_trait::async_trait;
use p2p::ProtocolHandler;
use p2p_identity::PeerId;
use p2p_mux::BoxedStream;
use p2p_protocol::{read_frame, ProtocolId};
use tokio::io::AsyncWriteExt;

use crate::events::ChatEvent;
use crate::invite::{FriendInvite, InviteDirection, InviteState};
use crate::model::{now_ms, parse_peer_id, validate_nickname, ChatError};
use crate::wire::{write_typed, AckFrame, ACK};
use crate::wire_invite::ack_for;
use crate::wire_invite::{deliver_frame, InviteFrame, ACCEPT, INVITE, REJECT};
use crate::{invite_api, ChatCore};

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
async fn on_invite(
    core: &Arc<ChatCore>,
    frame: &InviteFrame,
    peer: PeerId,
) -> Result<(), ChatError> {
    let peer_s = peer.to_string();
    let friends = core.store.friends_list()?;
    if friends.iter().any(|f| f.peer_id == peer_s) {
        // 重启自愈关键环：对端重发 INVITE 帧携带其最新 listen_addrs，先登记
        // 再回投 ACCEPT，否则回投仍拨旧地址、双向建簿永远无法收敛。
        for addr in &frame.addrs {
            if let Err(e) = core.node.add_peer_address(peer, addr) {
                tracing::warn!(peer = %peer_s, addr = %addr, error = %e, "自愈地址登记失败");
            }
        }
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
            invite_api::insert_friend(
                core,
                &peer_s,
                &pending.nickname,
                pending.addrs.clone(),
                pending.note.clone(),
            )?;
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
async fn on_accept(
    core: &Arc<ChatCore>,
    frame: &InviteFrame,
    peer: PeerId,
) -> Result<(), ChatError> {
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
            invite_api::insert_friend(
                core,
                &peer_s,
                &pending.nickname,
                pending.addrs.clone(),
                pending.note.clone(),
            )?;
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
async fn on_reject(
    core: &Arc<ChatCore>,
    _frame: &InviteFrame,
    peer: PeerId,
) -> Result<(), ChatError> {
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

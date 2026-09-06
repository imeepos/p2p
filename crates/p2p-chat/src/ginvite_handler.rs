//! /im/ginvite/1 入站 handler 与语义分支（GINVITE/GACCEPT/GREJECT 处理）。
//! 协议帧与客户端投递在 ginvite_wire.rs，门面在 ginvite_api.rs（行数红线拆分）。

use std::io;
use std::sync::Arc;

use async_trait::async_trait;
use p2p::ProtocolHandler;
use p2p_identity::PeerId;
use p2p_mux::BoxedStream;
use p2p_protocol::{read_frame, ProtocolId};
use tokio::io::AsyncWriteExt;

use crate::events::ChatEvent;
use crate::ginvite::{GroupInvite, GroupInviteDirection, GroupInviteState};
use crate::ginvite_wire::{ack_for, GinviteFrame, GACCEPT, GINVITE, GREJECT};
use crate::group_core::GroupCore;
use crate::group_store::{validate_group_name, GroupState};
use crate::model::{now_ms, parse_peer_id, ChatError};
use crate::wire::{write_typed, AckFrame, ACK};

pub(crate) struct GinviteHandler {
    core: Arc<GroupCore>,
    proto: ProtocolId,
}

/// 装配入口：注册 /im/ginvite/1 handler（由 Group::mount 统一挂载）。
pub(crate) fn register(core: &Arc<GroupCore>) -> Result<(), ChatError> {
    let proto =
        ProtocolId::new(crate::GINVITE_PROTOCOL).map_err(|e| ChatError::Protocol(e.to_string()))?;
    core.chat.node.handle_protocol(Arc::new(GinviteHandler {
        core: core.clone(),
        proto,
    }));
    Ok(())
}

#[async_trait]
impl ProtocolHandler for GinviteHandler {
    fn protocol(&self) -> ProtocolId {
        self.proto.clone()
    }

    async fn handle(&self, mut stream: BoxedStream) -> io::Result<()> {
        let outcome = self.handle_inbound(&mut stream).await;
        if let Err(e) = &outcome {
            tracing::warn!(error = %e, "/im/ginvite/1 入站帧校验失败，断流");
        }
        outcome
    }
}

impl GinviteHandler {
    async fn handle_inbound(&self, stream: &mut BoxedStream) -> io::Result<()> {
        let frame = read_frame(stream).await?;
        let Some((&kind, payload)) = frame.split_first() else {
            return Err(io::Error::other("入群邀请帧缺类型头"));
        };
        let parsed: GinviteFrame = serde_json::from_slice(payload)
            .map_err(|e| io::Error::other(format!("入群邀请帧 JSON 非法：{e}")))?;
        let local = self.core.chat.node.local_peer_id();
        let peer = validate_sender(&parsed, local)?;
        let outcome = match kind {
            GINVITE => on_invite(&self.core, &parsed, peer),
            GACCEPT => on_accept(&self.core, &parsed, peer).await,
            GREJECT => on_reject(&self.core, &parsed, peer),
            other => Err(ChatError::Protocol(format!(
                "未知入群邀请帧类型 {other:#04x}"
            ))),
        };
        if let Err(e) = outcome {
            tracing::warn!(peer = %parsed.peer, error = %e, "入群邀请帧处理失败，回 ACK nack");
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

/// 发端校验：peer 合法、非本机（同 /im/invite/1 纵深防御口径）。
fn validate_sender(frame: &GinviteFrame, local: PeerId) -> Result<PeerId, io::Error> {
    let peer = parse_peer_id(&frame.peer).map_err(|e| io::Error::other(e.to_string()))?;
    if peer == local {
        return Err(io::Error::other("入站入群邀请帧 peer 指向本机，疑似伪装"));
    }
    Ok(peer)
}

/// GINVITE（受邀者侧）：owner 绑定校验 → upsert in 条目（重复邀请幂等刷新回
/// pending）→ chat_group_invite(pending) 事件；已在群则告警幂等忽略。
fn on_invite(core: &Arc<GroupCore>, frame: &GinviteFrame, peer: PeerId) -> Result<(), ChatError> {
    let owner = parse_peer_id(&frame.owner)?;
    if owner != peer {
        return Err(ChatError::Protocol(
            "入站邀请帧 owner ≠ 发端 peer（发起 owner-only），疑似伪装".into(),
        ));
    }
    let group_name = validate_group_name(&frame.group_name)?;
    let local = core.chat.node.local_peer_id().to_string();
    let peer_s = peer.to_string();
    if core
        .store
        .group(&frame.group_id)
        .is_some_and(|g| g.state == GroupState::Active && g.members.contains(&local))
    {
        tracing::warn!(group_id = %frame.group_id, "受邀时本机已在群，忽略重复邀请");
        return Ok(());
    }
    let existing = core.chat.store.group_invites_list()?.into_iter().find(|i| {
        i.direction == GroupInviteDirection::In
            && i.group_id == frame.group_id
            && i.inviter == peer_s
    });
    let entry = GroupInvite {
        id: existing
            .map(|i| i.id)
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string()),
        group_id: frame.group_id.clone(),
        group_name,
        owner: peer_s.clone(),
        inviter: peer_s.clone(),
        invitee: local,
        note: frame.note.clone(),
        direction: GroupInviteDirection::In,
        state: GroupInviteState::Pending,
        ts_ms: now_ms(),
        delivered: true,
    };
    core.chat.store.upsert_group_invite(entry.clone())?;
    emit(core, entry);
    Ok(())
}

/// GACCEPT（owner 侧）：复用既有「邀请入群」路径（rev+1 推全体含新成员）后置
/// accepted；无待发邀请/群已解散仅告警幂等；已在群幂等跳过（重复回送安全）。
async fn on_accept(
    core: &Arc<GroupCore>,
    frame: &GinviteFrame,
    peer: PeerId,
) -> Result<(), ChatError> {
    let peer_s = peer.to_string();
    let local = core.chat.node.local_peer_id().to_string();
    if frame.owner != local {
        return Err(ChatError::Protocol(
            "收到同意帧但本机非该群 owner，拒绝".into(),
        ));
    }
    let Some(mut entry) = find_entry(core, GroupInviteDirection::Out, &frame.group_id, &peer_s)?
    else {
        tracing::warn!(peer = %peer_s, group_id = %frame.group_id, "收到无待发邀请的同意帧，幂等忽略");
        return Ok(());
    };
    if entry.state == GroupInviteState::Rejected {
        tracing::warn!(peer = %peer_s, group_id = %frame.group_id, "同意帧晚于受邀者拒绝，忽略");
        return Ok(());
    }
    let group = core.store.group(&frame.group_id);
    let Some(group) = group.filter(|g| g.owner == local) else {
        return Err(ChatError::NotFound(format!(
            "群不存在或 owner 绑定不符：{}",
            frame.group_id
        )));
    };
    if group.state != GroupState::Active {
        tracing::warn!(group_id = %frame.group_id, state = ?group.state, "群已非 active，同意帧忽略");
        return Ok(());
    }
    if !group.members.contains(&peer_s) {
        crate::group::Group { core: core.clone() }
            .group_invite(&frame.group_id, std::slice::from_ref(&peer_s))
            .await?;
    }
    if entry.state != GroupInviteState::Accepted {
        entry.state = GroupInviteState::Accepted;
        core.chat.store.patch_group_invite(&entry.id, |i| {
            i.state = GroupInviteState::Accepted;
        })?;
        emit(core, entry);
    }
    Ok(())
}

/// GREJECT（owner 侧）：out 条目置 rejected（幂等），reason 留日志可观测。
fn on_reject(core: &Arc<GroupCore>, frame: &GinviteFrame, peer: PeerId) -> Result<(), ChatError> {
    let peer_s = peer.to_string();
    let Some(mut entry) = find_entry(core, GroupInviteDirection::Out, &frame.group_id, &peer_s)?
    else {
        tracing::warn!(peer = %peer_s, group_id = %frame.group_id, "收到无待发邀请的拒绝帧，幂等忽略");
        return Ok(());
    };
    tracing::info!(
        peer = %peer_s,
        group_id = %frame.group_id,
        reason = frame.reason.as_deref().unwrap_or(""),
        "受邀者拒绝入群邀请"
    );
    if entry.state != GroupInviteState::Rejected {
        entry.state = GroupInviteState::Rejected;
        core.chat.store.patch_group_invite(&entry.id, |i| {
            i.state = GroupInviteState::Rejected;
        })?;
        emit(core, entry);
    }
    Ok(())
}

/// out 条目定位（group + 受邀者二元组）。
fn find_entry(
    core: &Arc<GroupCore>,
    direction: GroupInviteDirection,
    group_id: &str,
    invitee: &str,
) -> Result<Option<GroupInvite>, ChatError> {
    Ok(core
        .chat
        .store
        .group_invites_list()?
        .into_iter()
        .find(|i| i.direction == direction && i.group_id == group_id && i.invitee == invitee))
}

fn emit(core: &Arc<GroupCore>, invite: GroupInvite) {
    let _ = core.chat.events.send(ChatEvent::GroupInvite { invite });
}

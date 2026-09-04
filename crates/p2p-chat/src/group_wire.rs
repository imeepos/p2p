//! 线协议 /im/group/1（design im-group-design.md §3；wire-protocol.md §8.2 登记）。
//! 一流一事务，首帧类型决定事务：消息 G_ENVELOPE→（MEDIA_BEGIN→MEDIA_CHUNK×n）
//! →ACK；roster G_STATE→G_STATE_ACK；G_KICK/G_LEAVE 单向通知；坏帧断流 + 告警。
//! 身份缺口同 /im/chat/1：载荷声明发端，收端纵深校验（owner 绑定/rev 单调）。
//! roster/通知类型与校验在 group.rs（门面构造与线契约同文件，300 行红线再平衡）。

use std::io;
use std::sync::Arc;

use p2p::ProtocolHandler;
use p2p_identity::PeerId;
use p2p_mux::BoxedStream;
use p2p_protocol::{read_frame, ProtocolId};
use tokio::io::{AsyncRead, AsyncWriteExt};

use crate::group::{GroupKick, GroupLeave, GroupRoster, GroupStateAck};
use crate::group_core::GroupCore;
use crate::group_store::GroupMessage;
use crate::model::{
    parse_peer_id, validate_media, validate_text, ChatError, ChatKind, ChatMediaMeta,
};
use crate::wire::{write_typed, AckFrame, ACK};

pub(crate) const G_ENVELOPE: u8 = 0x01;
pub(crate) const G_STATE: u8 = 0x11;
pub(crate) const G_STATE_ACK: u8 = 0x12;
pub(crate) const G_KICK: u8 = 0x13;
pub(crate) const G_LEAVE: u8 = 0x14;

/// 线协议 ID（wire-protocol.md §8.2 登记，与 /im/chat/1 并存路由）。
pub const GROUP_PROTOCOL: &str = "/im/group/1";

/// 线上附件元数据（media 字段；path 为本地字段不跨网）。
#[derive(serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WireMedia {
    pub(crate) name: String,
    pub(crate) mime: String,
    pub(crate) size: u64,
}

/// 线上群信封（design §3.1）：sender = 作者 PeerId；status/path/acks 本地字段不跨网。
#[derive(serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WireGroupEnvelope {
    pub(crate) id: String,
    pub(crate) group_id: String,
    pub(crate) sender: String,
    pub(crate) kind: ChatKind,
    pub(crate) ts_ms: i64,
    pub(crate) text: Option<String>,
    pub(crate) media: Option<WireMedia>,
    pub(crate) reply_to: Option<String>,
}

impl WireGroupEnvelope {
    pub(crate) fn from_local(msg: &GroupMessage) -> Self {
        Self {
            id: msg.id.clone(),
            group_id: msg.group_id.clone(),
            sender: msg.sender_id.clone(),
            kind: msg.kind.clone(),
            ts_ms: msg.ts_ms,
            text: msg.text.clone(),
            media: msg.media.as_ref().map(|m| WireMedia {
                name: m.name.clone(),
                mime: m.mime.clone(),
                size: m.size,
            }),
            reply_to: msg.reply_to.clone(),
        }
    }

    /// 入站校验转本地信封（design §3.3）：sender base58 且非本机；kind/text/media 同 1:1。
    pub(crate) fn into_inbound(self, local: PeerId) -> Result<GroupMessage, ChatError> {
        let sender_id = parse_peer_id(&self.sender)?;
        if sender_id == local {
            return Err(ChatError::Protocol(
                "入站群信封 sender 指向本机，疑似伪装".into(),
            ));
        }
        let media = match (&self.kind, self.media) {
            (ChatKind::Text, None) => None,
            (ChatKind::Text, Some(_)) => {
                return Err(ChatError::InvalidMedia("text 消息携带附件，拒绝".into()));
            }
            (kind, None) => {
                return Err(ChatError::InvalidMedia(format!("{kind} 消息缺附件，拒绝")))
            }
            (kind, Some(m)) => {
                validate_media(kind, &m.mime, m.size)?;
                Some(ChatMediaMeta {
                    name: m.name,
                    mime: m.mime,
                    size: m.size,
                    path: None,
                })
            }
        };
        let text = match &self.kind {
            ChatKind::Text => Some(validate_text(self.text.as_deref().unwrap_or_default())?),
            _ => None,
        };
        Ok(GroupMessage {
            id: self.id,
            group_id: self.group_id,
            sender_id: self.sender,
            kind: self.kind,
            ts_ms: self.ts_ms,
            text,
            media,
            status: crate::model::ChatStatus::Delivered,
            acks: Vec::new(),
            reply_to: self.reply_to,
        })
    }
}

/// 读一帧并校验类型头，不符即协议错误（断流由 handler 统一告警）。
pub(crate) async fn read_typed(
    r: &mut (impl AsyncRead + Unpin + Send),
    want: u8,
    what: &str,
) -> io::Result<Vec<u8>> {
    let frame = read_frame(r).await?;
    let Some((&kind, payload)) = frame.split_first() else {
        return Err(io::Error::other(format!("{what} 帧缺类型头")));
    };
    if kind != want {
        return Err(io::Error::other(format!(
            "期望 {what}({want:#04x})，收到 {kind:#04x}"
        )));
    }
    Ok(payload.to_vec())
}

/// 入站 /im/group/1 handler：首帧类型分发（一流一事务）。
pub(crate) struct GroupHandler {
    core: Arc<GroupCore>,
    proto: ProtocolId,
}

impl GroupHandler {
    pub(crate) fn new(core: Arc<GroupCore>, proto: ProtocolId) -> Self {
        Self { core, proto }
    }

    async fn dispatch(&self, stream: &mut BoxedStream) -> io::Result<()> {
        let frame = read_frame(stream).await?;
        let Some((&kind, payload)) = frame.split_first() else {
            return Err(io::Error::other("首帧缺类型头"));
        };
        match kind {
            G_ENVELOPE => self.handle_message(stream, payload).await,
            G_STATE => self.handle_roster(stream, payload).await,
            G_KICK | G_LEAVE => Self::handle_notice(&self.core, kind, payload).await,
            other => Err(io::Error::other(format!("未知群帧类型 {other:#04x}"))),
        }
    }

    /// 消息事务：不在册群回 unknown_group；sender ∉ 成员断流；去重后落盘 + 事件。
    async fn handle_message(&self, stream: &mut BoxedStream, payload: &[u8]) -> io::Result<()> {
        let wire: WireGroupEnvelope = serde_json::from_slice(payload)
            .map_err(|e| io::Error::other(format!("群信封 JSON 非法：{e}")))?;
        let local = self.core.chat.node.local_peer_id();
        let mut msg = wire.into_inbound(local).map_err(io::Error::other)?;
        if !self
            .core
            .store
            .groups_list()
            .iter()
            .any(|g| g.group_id == msg.group_id)
        {
            tracing::warn!(group_id = %msg.group_id, id = %msg.id, "收到未知群消息，回 unknown_group");
            let ack = AckFrame {
                id: msg.id.clone(),
                ok: false,
                reason: Some("unknown_group".into()),
            };
            write_typed(
                stream,
                ACK,
                &serde_json::to_vec(&ack).map_err(io::Error::other)?,
            )
            .await?;
            return stream.flush().await;
        }
        let group = self.core.store.group(&msg.group_id);
        if !group.is_some_and(|g| g.members.contains(&msg.sender_id)) {
            return Err(io::Error::other(format!(
                "sender {} 非群 {} 在册成员，疑似伪装",
                msg.sender_id, msg.group_id
            )));
        }
        if let Some(meta) = msg.media.clone() {
            let path = self
                .core
                .receive_media(stream, &msg.group_id, &msg.id, &meta)
                .await?;
            if let Some(m) = msg.media.as_mut() {
                m.path = Some(path.to_string_lossy().into_owned());
            }
        }
        let dup = self
            .core
            .store
            .history(&msg.group_id)
            .iter()
            .any(|m| m.id == msg.id);
        let ack = AckFrame {
            id: msg.id.clone(),
            ok: true,
            reason: None,
        };
        write_typed(
            stream,
            ACK,
            &serde_json::to_vec(&ack).map_err(io::Error::other)?,
        )
        .await?;
        stream.flush().await?;
        if !dup {
            self.core
                .store
                .append_message(&msg)
                .map_err(io::Error::other)?;
            let group_id = msg.group_id.clone();
            self.core.emit(crate::group_model::GroupEvent::Message {
                group_id,
                message: msg,
            });
        }
        Ok(())
    }

    /// roster 事务：纵深校验 + owner 绑定/rev 收敛 → G_STATE_ACK（拒收回 ok=false 再断流）。
    async fn handle_roster(&self, stream: &mut BoxedStream, payload: &[u8]) -> io::Result<()> {
        let roster: GroupRoster = serde_json::from_slice(payload)
            .map_err(|e| io::Error::other(format!("roster JSON 非法：{e}")))?;
        let local = self.core.chat.node.local_peer_id();
        let applied = roster
            .validate(local)
            .and_then(|()| self.core.apply_roster(&roster));
        let ack = GroupStateAck {
            group_id: roster.group_id,
            rev: roster.rev,
            ok: applied.is_ok(),
            reason: applied.as_ref().err().cloned(),
        };
        write_typed(
            stream,
            G_STATE_ACK,
            &serde_json::to_vec(&ack).map_err(io::Error::other)?,
        )
        .await?;
        stream.flush().await?;
        applied.map_err(io::Error::other)
    }

    /// G_KICK/G_LEAVE 单向通知应用（design §5）。
    async fn handle_notice(core: &GroupCore, kind: u8, payload: &[u8]) -> io::Result<()> {
        match kind {
            G_KICK => {
                let k: GroupKick = serde_json::from_slice(payload)
                    .map_err(|e| io::Error::other(format!("G_KICK JSON 非法：{e}")))?;
                core.apply_kick(&k).map_err(io::Error::other)
            }
            _ => {
                let l: GroupLeave = serde_json::from_slice(payload)
                    .map_err(|e| io::Error::other(format!("G_LEAVE JSON 非法：{e}")))?;
                core.apply_leave(&l).await.map_err(io::Error::other)
            }
        }
    }
}

#[async_trait::async_trait]
impl ProtocolHandler for GroupHandler {
    fn protocol(&self) -> ProtocolId {
        self.proto.clone()
    }

    async fn handle(&self, mut stream: BoxedStream) -> io::Result<()> {
        let outcome = self.dispatch(&mut stream).await;
        if let Err(e) = &outcome {
            tracing::warn!(error = %e, "/im/group/1 入站帧校验失败，断流");
        }
        outcome
    }
}

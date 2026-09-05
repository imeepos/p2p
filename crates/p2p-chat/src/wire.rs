//! 线协议 /im/chat/1 帧编解码与入站 handler（design §3；wire-protocol.md §8 登记）。
//!
//! 帧载荷首字节 = 类型头（与 chunked 同风格），其余为 JSON 或原始字节：
//! ENVELOPE 0x01 / MEDIA_BEGIN 0x02 / MEDIA_CHUNK 0x03 / ACK 0x04。
//! 时序：ENVELOPE →（MEDIA_BEGIN → MEDIA_CHUNK×n）→ ACK；任意帧校验失败断流告警。
//!
//! 对端身份说明：底座 handler 拿不到对端 PeerId（serve.rs 分发不携带 peer），
//! 故线上 peer 字段承载发端自身 PeerId；收端校验其合法、非本机且 sender 为 me
//! （sender=them 即冒充本机）——内核只读约束下的纵深防御上限，流安全由底座保证。

use std::io;
use std::sync::Arc;

use async_trait::async_trait;
use p2p::ProtocolHandler;
use p2p_identity::PeerId;
use p2p_mux::BoxedStream;
use p2p_protocol::{read_frame, write_frame, ProtocolId};
use tokio::io::{AsyncWrite, AsyncWriteExt};

use crate::events::ChatEvent;
use crate::model::{
    parse_peer_id, validate_media, validate_text, ChatEnvelope, ChatError, ChatKind, ChatMediaMeta,
    ChatStatus, Sender,
};
use crate::ChatCore;

pub(crate) const ENVELOPE: u8 = 0x01;
pub(crate) const MEDIA_BEGIN: u8 = 0x02;
pub(crate) const MEDIA_CHUNK: u8 = 0x03;
pub(crate) const ACK: u8 = 0x04;
/// 单分片数据上限（帧长 1MiB - 类型头 1 字节，对齐 CHUNK_DATA_SIZE）。
pub(crate) const CHUNK_LEN: usize = 1_048_575;

/// 线上信封：peer = 发端自身 PeerId；status/path 为本地字段不跨网。
/// fromAddrs 加法字段（F1 地址自学习）：发端声明地址，收端回写好友簿；
/// serde(default) 保证旧对端缺字段可读，旧对端收新字段也忽略（双向兼容）。
#[derive(serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WireEnvelope {
    id: String,
    peer: String,
    sender: Sender,
    kind: ChatKind,
    #[serde(rename = "tsMs")]
    ts_ms: i64,
    text: Option<String>,
    media: Option<WireMedia>,
    reply_to: Option<String>,
    #[serde(default)]
    pub(crate) from_addrs: Option<Vec<String>>,
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct WireMedia {
    name: String,
    mime: String,
    size: u64,
}

/// 媒体头（MEDIA_BEGIN 单帧载荷）。
#[derive(serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MediaBegin {
    pub(crate) len: u64,
    pub(crate) name: String,
    pub(crate) mime: String,
    pub(crate) kind: ChatKind,
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AckFrame {
    pub(crate) id: String,
    pub(crate) ok: bool,
    pub(crate) reason: Option<String>,
}

impl WireEnvelope {
    /// 发端声明地址取 serve 发布的 advertised（空则不携带）：一次性进程的
    /// 即弃监听地址禁止上 wire，防对端好友簿被污染。
    pub(crate) fn from_outbound(env: &ChatEnvelope, local: PeerId, from_addrs: Vec<String>) -> Self {
        Self {
            id: env.id.clone(),
            peer: local.to_string(),
            sender: Sender::Me,
            kind: env.kind.clone(),
            ts_ms: env.ts_ms,
            text: env.text.clone(),
            media: env.media.as_ref().map(|m| WireMedia {
                name: m.name.clone(),
                mime: m.mime.clone(),
                size: m.size,
            }),
            reply_to: env.reply_to.clone(),
            from_addrs: if from_addrs.is_empty() {
                None
            } else {
                Some(from_addrs)
            },
        }
    }

    /// 入站校验并转存储信封：sender 必须为 me、peer 合法且非本机。
    pub(crate) fn into_inbound(self, local: PeerId) -> Result<ChatEnvelope, ChatError> {
        if self.sender != Sender::Me {
            return Err(ChatError::Protocol(
                "入站信封 sender 非 me（对端视角），疑似伪装".into(),
            ));
        }
        let peer_id = parse_peer_id(&self.peer)?;
        if peer_id == local {
            return Err(ChatError::Protocol(
                "入站信封 peer 指向本机，疑似伪装".into(),
            ));
        }
        match self.kind {
            ChatKind::Text => {
                let text = validate_text(self.text.as_deref().unwrap_or_default())?;
                if self.media.is_some() {
                    return Err(ChatError::InvalidMedia("text 消息携带附件，拒绝".into()));
                }
                Ok(ChatEnvelope {
                    id: self.id,
                    peer: self.peer,
                    sender: Sender::Them,
                    kind: ChatKind::Text,
                    ts_ms: self.ts_ms,
                    text: Some(text),
                    media: None,
                    status: ChatStatus::Delivered,
                    reply_to: self.reply_to,
                })
            }
            kind => {
                let m = self
                    .media
                    .ok_or_else(|| ChatError::InvalidMedia(format!("{kind} 消息缺附件，拒绝")))?;
                validate_media(&kind, &m.mime, m.size)?;
                Ok(ChatEnvelope {
                    id: self.id,
                    peer: self.peer,
                    sender: Sender::Them,
                    kind,
                    ts_ms: self.ts_ms,
                    text: None,
                    media: Some(ChatMediaMeta {
                        name: m.name,
                        mime: m.mime,
                        size: m.size,
                        path: None,
                    }),
                    status: ChatStatus::Delivered,
                    reply_to: self.reply_to,
                })
            }
        }
    }
}

/// 写一帧：类型头 + 载荷（帧长受 read_frame/write_frame 1MiB 上限约束）。
pub(crate) async fn write_typed(
    w: &mut (impl AsyncWrite + Unpin + Send),
    kind: u8,
    payload: &[u8],
) -> io::Result<()> {
    let mut frame = Vec::with_capacity(1 + payload.len());
    frame.push(kind);
    frame.extend_from_slice(payload);
    write_frame(w, &frame).await
}

/// 读对端 ACK 帧；类型头非 ACK 即断流报错。
pub(crate) async fn read_ack(
    r: &mut (impl tokio::io::AsyncRead + Unpin + Send),
) -> io::Result<AckFrame> {
    let frame = read_frame(r).await?;
    let Some((&kind, payload)) = frame.split_first() else {
        return Err(io::Error::other("ACK 帧缺类型头"));
    };
    if kind != ACK {
        return Err(io::Error::other(format!(
            "期望 ACK(0x04)，收到 {kind:#04x}"
        )));
    }
    serde_json::from_slice(payload).map_err(io::Error::other)
}

/// 入站 /im/chat/1 handler：读信封 → 收媒体 → 回 ACK → 落盘 → 发事件。
pub(crate) struct ChatHandler {
    core: Arc<ChatCore>,
    proto: ProtocolId,
}

impl ChatHandler {
    pub(crate) fn new(core: Arc<ChatCore>, proto: ProtocolId) -> Self {
        Self { core, proto }
    }
}

#[async_trait]
impl ProtocolHandler for ChatHandler {
    fn protocol(&self) -> ProtocolId {
        self.proto.clone()
    }

    async fn handle(&self, mut stream: BoxedStream) -> io::Result<()> {
        let outcome = self.handle_inbound(&mut stream).await;
        if let Err(e) = &outcome {
            tracing::warn!(error = %e, "/im/chat/1 入站帧校验失败，断流");
        }
        outcome
    }
}

impl ChatHandler {
    async fn handle_inbound(&self, stream: &mut BoxedStream) -> io::Result<()> {
        let frame = read_frame(stream).await?;
        let Some((&kind, payload)) = frame.split_first() else {
            return Err(io::Error::other("信封帧缺类型头"));
        };
        if kind != ENVELOPE {
            return Err(io::Error::other(format!(
                "首帧必须为 ENVELOPE(0x01)，收到 {kind:#04x}"
            )));
        }
        let wire: WireEnvelope = serde_json::from_slice(payload)
            .map_err(|e| io::Error::other(format!("信封 JSON 非法：{e}")))?;
        let learned = wire.from_addrs.clone().unwrap_or_default();
        let local = self.core.node.local_peer_id();
        let mut env = wire
            .into_inbound(local)
            .map_err(|e| io::Error::other(e.to_string()))?;
        if !learned.is_empty() {
            crate::addr_learn::learn_friend_addrs(&self.core, &env.peer, &learned);
        }
        let dup = self.core.store.has_message(&env.peer, &env.id);
        if let Some(media) = env.media.as_ref() {
            let path = self
                .core
                .receive_media(stream, &env.peer, &env.id, media)
                .await?;
            if let Some(m) = env.media.as_mut() {
                m.path = Some(path.to_string_lossy().into_owned());
            }
        }
        let ack = AckFrame {
            id: env.id.clone(),
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
            self.core.store.append_message(&env)?;
            let _ = self.core.events.send(ChatEvent::ChatMessage {
                peer: env.peer.clone(),
                message: env,
            });
        }
        Ok(())
    }
}


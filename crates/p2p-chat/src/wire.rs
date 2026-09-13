//! 线协议 /im/chat/1 帧编解码与入站 handler（design §3；wire-protocol.md §8 登记）。
//!
//! 帧载荷首字节 = 类型头（与 chunked 同风格）：ENVELOPE 0x01 / MEDIA_BEGIN 0x02 /
//! MEDIA_CHUNK 0x03 / ACK 0x04；时序与断流纪律见 §8.1（wire-protocol.md）登记。
//! 信封形状与出入站转换见 [wire_envelope]（本文件 re-export 保持引用路径不变）。

use std::io;
use std::sync::Arc;

use async_trait::async_trait;
use p2p::ProtocolHandler;
use p2p_identity::PeerId;
use p2p_mux::BoxedStream;
use p2p_protocol::{read_frame, write_frame, ProtocolId};
use tokio::io::{AsyncWrite, AsyncWriteExt};

use crate::events::ChatEvent;
use crate::model::{parse_peer_id, ChatEnvelope, ChatKind};
use crate::ChatCore;

pub(crate) use crate::wire_envelope::WireEnvelope;

pub(crate) const ENVELOPE: u8 = 0x01;
pub(crate) const MEDIA_BEGIN: u8 = 0x02;
pub(crate) const MEDIA_CHUNK: u8 = 0x03;
pub(crate) const ACK: u8 = 0x04;
/// 单分片数据上限（帧长 1MiB - 类型头 1 字节，对齐 CHUNK_DATA_SIZE）。
pub(crate) const CHUNK_LEN: usize = 1_048_575;

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
    /// 读首帧并校验转存储信封；返回信封、发端 PeerId（闸判定键）与声明地址。
    async fn read_envelope(
        core: &ChatCore,
        stream: &mut BoxedStream,
    ) -> io::Result<(ChatEnvelope, PeerId, Vec<String>)> {
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
        let env = wire
            .into_inbound(core.node.local_peer_id())
            .map_err(|e| io::Error::other(e.to_string()))?;
        let peer = parse_peer_id(&env.peer).map_err(|e| io::Error::other(e.to_string()))?;
        Ok((env, peer, learned))
    }

    /// 入站主链（Amended A-1）：帧校验 → 准入闸（send→attachment）→ 地址学习 →
    /// 收媒体 → ACK → 落库 → 事件。闸拒 = 整帧拒收：不落库、不广播、不回帧。
    async fn handle_inbound(&self, stream: &mut BoxedStream) -> io::Result<()> {
        let (mut env, peer, learned) = Self::read_envelope(&self.core, stream).await?;
        crate::gate::admit_message(&self.core.gate, &peer, env.media.is_some())
            .map_err(|r| io::Error::other(format!("入站准入拒绝: {}", r.reason)))?;
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

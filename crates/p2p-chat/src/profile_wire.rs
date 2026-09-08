//! 线协议 /im/profile/1 对端节点资料查询（wire-protocol.md §8.5 登记）。
//!
//! 帧纪律同 §8.3/§8.4：payload 首字节 = 类型头，其余为 JSON；一条流一次
//! 问答：GET 0x01 {id} → RESP 0x02 {id, ok, profile?, reason?}。不落盘、
//! 不产生事件，纯读语义；响应内容为对端自报资料，展示层自行取舍。

use std::io;
use std::sync::Arc;

use async_trait::async_trait;
use p2p::ProtocolHandler;
use p2p_mux::BoxedStream;
use p2p_protocol::{read_frame, ProtocolId};
use tokio::io::AsyncWriteExt;

use crate::model::ChatError;
use crate::profile::PeerProfile;
use crate::wire::write_typed;
use crate::ChatCore;

pub(crate) const GET: u8 = 0x01;
pub(crate) const RESP: u8 = 0x02;

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GetFrame {
    pub(crate) id: String,
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RespFrame {
    pub(crate) id: String,
    pub(crate) ok: bool,
    pub(crate) profile: Option<PeerProfile>,
    pub(crate) reason: Option<String>,
}

pub(crate) struct ProfileHandler {
    core: Arc<ChatCore>,
    proto: ProtocolId,
}

impl ProfileHandler {
    pub(crate) fn new(core: Arc<ChatCore>, proto: ProtocolId) -> Self {
        Self { core, proto }
    }
}

#[async_trait]
impl ProtocolHandler for ProfileHandler {
    fn protocol(&self) -> ProtocolId {
        self.proto.clone()
    }

    async fn handle(&self, mut stream: BoxedStream) -> io::Result<()> {
        let outcome = self.respond(&mut stream).await;
        if let Err(e) = &outcome {
            tracing::warn!(error = %e, "/im/profile/1 入站帧校验失败，断流");
        }
        outcome
    }
}

impl ProfileHandler {
    async fn respond(&self, stream: &mut BoxedStream) -> io::Result<()> {
        let frame = read_frame(stream).await?;
        let Some((&kind, payload)) = frame.split_first() else {
            return Err(io::Error::other("资料查询帧缺类型头"));
        };
        if kind != GET {
            return Err(io::Error::other(format!(
                "未知资料帧类型 {kind:#04x}，断流"
            )));
        }
        let get: GetFrame = serde_json::from_slice(payload)
            .map_err(|e| io::Error::other(format!("资料查询帧 JSON 非法：{e}")))?;
        let profile = (self.core.local_profile)();
        if let Err(e) = profile.validate() {
            // 本机资料越界不应中断节点，降级回空资料并留告警
            tracing::warn!(error = %e, "本机节点资料越界，按空资料应答");
            return write_resp(
                stream,
                &RespFrame {
                    id: get.id,
                    ok: true,
                    profile: Some(PeerProfile::default()),
                    reason: None,
                },
            )
            .await;
        }
        write_resp(
            stream,
            &RespFrame {
                id: get.id,
                ok: true,
                profile: Some(profile),
                reason: None,
            },
        )
        .await
    }
}

pub(crate) async fn write_resp(stream: &mut BoxedStream, resp: &RespFrame) -> io::Result<()> {
    let bytes = serde_json::to_vec(resp).map_err(io::Error::other)?;
    write_typed(stream, RESP, &bytes).await?;
    stream.flush().await
}

/// 客户端读回应：id 匹配 + ok 校验 + 资料长度防线。
pub(crate) fn resp_into_profile(
    resp: RespFrame,
    expect_id: &str,
) -> Result<PeerProfile, ChatError> {
    if resp.id != expect_id {
        return Err(ChatError::Protocol(format!(
            "资料回应 id 不匹配：{} ≠ {expect_id}",
            resp.id
        )));
    }
    if !resp.ok {
        return Err(ChatError::Protocol(format!(
            "对端拒绝资料查询：{}",
            resp.reason.as_deref().unwrap_or("")
        )));
    }
    let profile = resp
        .profile
        .ok_or_else(|| ChatError::Protocol("资料回应缺 profile 载荷".into()))?;
    profile.validate()?;
    Ok(profile)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resp_into_profile_rejects_id_mismatch_and_nack() {
        let resp = RespFrame {
            id: "a".into(),
            ok: true,
            profile: Some(PeerProfile::default()),
            reason: None,
        };
        assert!(resp_into_profile(resp, "b").is_err());

        let nack = RespFrame {
            id: "b".into(),
            ok: false,
            profile: None,
            reason: Some("no".into()),
        };
        let err = resp_into_profile(nack, "b").unwrap_err().to_string();
        assert!(err.contains("no"));
    }

    #[test]
    fn resp_missing_profile_is_protocol_error() {
        let resp = RespFrame {
            id: "b".into(),
            ok: true,
            profile: None,
            reason: None,
        };
        assert!(resp_into_profile(resp, "b").is_err());
    }
}

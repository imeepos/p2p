//! /im/profile/1 客户端查询：连接 → 开流 → GET → 读 RESP（peer 串行锁互斥）。

use std::time::Duration;

use tokio::io::AsyncWriteExt;

use crate::model::{parse_peer_id, ChatError};
use crate::profile::PeerProfile;
use crate::profile_wire::{resp_into_profile, GetFrame, RespFrame, GET, RESP};
use crate::ChatCore;

/// 一问一答全程超时：对端应答只读本机文件，10s 足够含建连。
const PROFILE_TIMEOUT: Duration = Duration::from_secs(10);

/// 查询对端节点资料；对端未设置时返回全空 PeerProfile（非错误）。
pub(crate) async fn peer_profile(core: &ChatCore, peer: &str) -> Result<PeerProfile, ChatError> {
    let pid = parse_peer_id(peer)?;
    if pid == core.node.local_peer_id() {
        return Err(ChatError::SelfPeer(peer.to_string()));
    }
    let _guard = core.peer_guard(peer).await;
    core.node
        .connect(pid)
        .await
        .map_err(|e| ChatError::ConnectFailed(format!("连接 {peer} 失败：{e}")))?;
    let proto = p2p::ProtocolId::new(crate::PROFILE_PROTOCOL)
        .map_err(|e| ChatError::Protocol(e.to_string()))?;
    let mut stream = core
        .node
        .new_stream(pid, proto)
        .await
        .map_err(|e| ChatError::StreamFailed(format!("开流失败：{e}")))?;
    let get = GetFrame {
        id: uuid::Uuid::new_v4().to_string(),
    };
    let call = async {
        let bytes = serde_json::to_vec(&get).map_err(ChatError::Json)?;
        let mut frame = Vec::with_capacity(1 + bytes.len());
        frame.push(GET);
        frame.extend_from_slice(&bytes);
        p2p_protocol::write_frame(&mut stream, &frame).await?;
        stream.flush().await?;
        let raw = p2p_protocol::read_frame(&mut stream).await?;
        let Some((&kind, payload)) = raw.split_first() else {
            return Err(ChatError::Protocol("资料回应帧缺类型头".into()));
        };
        if kind != RESP {
            return Err(ChatError::Protocol(format!(
                "资料回应类型 {kind:#04x} ≠ {RESP:#04x}"
            )));
        }
        let resp: RespFrame = serde_json::from_slice(payload).map_err(ChatError::Json)?;
        resp_into_profile(resp, &get.id)
    };
    match tokio::time::timeout(PROFILE_TIMEOUT, call).await {
        Ok(res) => {
            if let Err(e) = &res {
                tracing::warn!(peer = %peer, error = %e, "对端资料查询失败");
            }
            res
        }
        Err(_) => Err(ChatError::ProfileUnavailable(format!(
            "等待对端资料回应超时（{PROFILE_TIMEOUT:?}）"
        ))),
    }
}

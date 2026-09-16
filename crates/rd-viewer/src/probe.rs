//! 握手探测：hello → hello_ack（含 awaiting_approval 等拒绝原因上抛）即关流。

use std::sync::Arc;

use p2p::Node;
use rd_wire::io::{recv_control, send_control};
use rd_wire::{ControlMsg, Role};

use crate::ViewerError;

/// 仅握手不建会话：CLI 连通性探测 / 审批语义验证用。
pub async fn probe(
    node: Arc<Node>,
    peer: p2p::PeerId,
    session_id: String,
) -> Result<String, ViewerError> {
    let mut stream = node
        .new_stream(peer, rd_wire::control_protocol_id()?)
        .await?;
    send_control(
        &mut stream,
        &ControlMsg::Hello {
            v: rd_wire::PROTOCOL_VERSION,
            role: Role::Viewer,
            session_id: session_id.clone(),
            caps: rd_wire::Caps {
                audio: false,
                file: true,
                clipboard: true,
            },
        },
    )
    .await?;
    let ack = recv_control(&mut stream).await?;
    match ack {
        ControlMsg::HelloAck {
            ok: true,
            session_id: sid,
            ..
        } if sid == session_id => Ok(sid),
        ControlMsg::HelloAck {
            ok: false, reason, ..
        } => Err(ViewerError::Rejected(
            reason.unwrap_or_else(|| "no reason".into()),
        )),
        other => Err(ViewerError::Decode(format!("unexpected ack: {other:?}"))),
    }
}

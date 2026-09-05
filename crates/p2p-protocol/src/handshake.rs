//! 开流/收流两侧的协议握手助手（design §5.1），供 swarm 与 request-response 复用。

use std::io;

use p2p_identity::PeerId;
use p2p_mux::BoxedStream;
use tokio::io::AsyncWriteExt;

use crate::{
    flatten_io, read_protocol_id, write_protocol_id, HandlerRegistry, ProtocolError, ProtocolId,
};

/// 开流侧：新流首帧写协议 ID 并冲刷，之后流即可承载业务帧。
pub async fn open_with_protocol(
    mut stream: BoxedStream,
    id: &ProtocolId,
) -> io::Result<BoxedStream> {
    write_protocol_id(&mut stream, id).await?;
    stream.flush().await?;
    Ok(stream)
}

/// 收流侧：读协议 ID 查注册表分发；handler 拥有该流直到返回。
/// 未注册协议返回 [ProtocolError::UnsupportedProtocol]，不猜测降级（design §5.2）。
/// 无对端身份上下文（盲拨应答等）的裸流入口，handler 走旧签名 [crate::ProtocolHandler::handle]。
pub async fn dispatch_inbound(
    stream: BoxedStream,
    registry: &HandlerRegistry,
) -> Result<(), ProtocolError> {
    dispatch_inbound_with_peer(stream, None, registry).await
}

/// 收流侧（带对端身份）：swarm serve 路径唯一入口，peer 必须取自安全握手
/// 互认结果（SecureConn.remote），handler 经 [crate::ProtocolHandler::handle_inbound]
/// 确定性拿到真实远端身份，禁止上层以在线集推断绕行。
pub async fn dispatch_inbound_with_peer(
    stream: BoxedStream,
    peer: Option<PeerId>,
    registry: &HandlerRegistry,
) -> Result<(), ProtocolError> {
    let mut stream = stream;
    let id = match read_protocol_id(&mut stream).await {
        Ok(id) => id,
        Err(e) => return Err(flatten_io(e)),
    };
    match registry.get(&id) {
        Some(handler) => {
            let served = match peer {
                Some(peer) => handler.handle_inbound(peer, stream).await,
                None => handler.handle(stream).await,
            };
            served.map_err(flatten_io)
        }
        None => Err(ProtocolError::UnsupportedProtocol(id)),
    }
}

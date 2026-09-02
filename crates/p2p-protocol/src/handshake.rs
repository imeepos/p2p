//! 开流/收流两侧的协议握手助手（design §5.1），供 swarm 与 request-response 复用。

use std::io;

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
pub async fn dispatch_inbound(
    stream: BoxedStream,
    registry: &HandlerRegistry,
) -> Result<(), ProtocolError> {
    let mut stream = stream;
    let id = match read_protocol_id(&mut stream).await {
        Ok(id) => id,
        Err(e) => return Err(flatten_io(e)),
    };
    match registry.get(&id) {
        Some(handler) => handler.handle(stream).await.map_err(flatten_io),
        None => Err(ProtocolError::UnsupportedProtocol(id)),
    }
}

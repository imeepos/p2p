//! /p2p-lab/echo/1 回声 handler（design §9 业务协议接入示例）。
//!
//! 收一帧即原样回一帧（与 request-response 一问一答契约一致）。
//! 失败路径返回 io 错误，让分发层按协议违规上报，禁止静默吞错。

use std::io;

use p2p::ProtocolHandler;
use p2p_mux::BoxedStream;
use p2p_protocol::{read_frame, write_frame, ProtocolId};

/// node 子命令注册的 echo 协议 ID。
pub const ECHO_PROTOCOL: &str = "/p2p-lab/echo/1";

/// ping 载荷（无业务语义，仅测 RTT）。
pub const PING_PAYLOAD: &[u8] = b"p2p-ping";

/// 回声 handler：读一帧写回同一帧，流关闭即完成。
pub struct EchoHandler;

#[async_trait::async_trait]
impl ProtocolHandler for EchoHandler {
    fn protocol(&self) -> ProtocolId {
        ProtocolId::new(ECHO_PROTOCOL).expect("built-in echo protocol id is valid")
    }

    async fn handle(&self, mut stream: BoxedStream) -> io::Result<()> {
        let frame = read_frame(&mut stream).await?;
        write_frame(&mut stream, &frame).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use p2p::ProtocolId;
    use p2p_protocol::{open_with_protocol, read_frame, read_protocol_id, write_frame};

    /// 回声 handler 收到一帧原样返回（一问一答闭环）。
    #[tokio::test]
    async fn echo_roundtrips_payload() {
        let (client, server) = tokio::io::duplex(4096);
        let id = ProtocolId::new(ECHO_PROTOCOL).expect("valid id");

        // 模拟 swarm 真实分发：服务端先消费协议 ID 首帧，再交 handler（design §5.1）
        let handler_task = tokio::spawn(async move {
            let mut server = Box::new(server);
            let _first = read_protocol_id(&mut server).await.expect("read proto id");
            EchoHandler.handle(server).await.expect("echo ok");
        });

        // client 侧：开流写协议 ID -> 写载荷 -> 读回帧
        let mut stream = open_with_protocol(Box::new(client), &id)
            .await
            .expect("open with protocol");
        let payload = b"roundtrip-payload".to_vec();
        write_frame(&mut stream, &payload).await.expect("write req");
        let reply = read_frame(&mut stream).await.expect("read reply");
        assert_eq!(reply, payload);

        handler_task.await.expect("handler task clean");
    }

    /// 对端直接关流（EOF）：handler 读帧报错，把错误抛还给分发层，不 panic。
    #[tokio::test]
    async fn echo_closes_cleanly_on_eof() {
        let (client, server) = tokio::io::duplex(4096);
        let handler_task = tokio::spawn(async move {
            EchoHandler
                .handle(Box::new(server))
                .await
                .expect_err("eof must surface as error");
        });
        drop(client);
        handler_task.await.expect("handler clean");
    }
}

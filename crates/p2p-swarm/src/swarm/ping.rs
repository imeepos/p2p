//! 内置 liveness ping（E6）：design §5.4 控制协议 /p2p-base/ping/1 的 swarm 实现。
//!
//! 选型说明（任务要求二选一并写明理由）：探活走协议层 ping，不用 QUIC keepalive——
//! 1. 传输无关：QUIC keepalive 只覆盖 QUIC 直连；本栈兜底路径 TCP 与中继电路
//!    均探测不到。协议 ping 在 mux 流上往返，QUIC/TCP/中继电路语义一致。
//! 2. 端到端：验证对端协议栈确实在服务；连接半死（对端进程 hang、驱动停转）
//!    时 keepalive 仍可能存活，ping 能发现。
//!
//! 协议 ID 复用 relay crate 已登记常量（单一事实源，避免两处漂移）。
//! 应答侧不抢占用户 handler：registry 已含该 ID 时跳过注入（测试据此模拟失聪对端）。

use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use p2p_mux::BoxedStream;
use p2p_protocol::{
    open_with_protocol, read_frame, write_frame, HandlerRegistry, ProtocolHandler, ProtocolId,
};

use super::Mux;

/// design §5.4 内置 ping 协议 ID；与 p2p_relay::proto_ids::PING 同源。
pub const PING_PROTOCOL: &str = p2p_relay::proto_ids::PING;

fn ping_id() -> ProtocolId {
    ProtocolId::new(PING_PROTOCOL).expect("valid ping protocol id")
}

/// 应答侧：读一帧原样回一帧，返回即关流。帧上限由 read_frame 统一把关。
pub(crate) struct PingHandler;

#[async_trait::async_trait]
impl ProtocolHandler for PingHandler {
    fn protocol(&self) -> ProtocolId {
        ping_id()
    }

    async fn handle(&self, mut stream: BoxedStream) -> std::io::Result<()> {
        let req = read_frame(&mut stream).await?;
        write_frame(&mut stream, &req).await
    }
}

/// registry 无 ping handler 时注入内置实现；用户 handler 优先。
pub(crate) fn registry_with_ping(registry: Arc<HandlerRegistry>) -> Arc<HandlerRegistry> {
    if registry.get(&ping_id()).is_some() {
        return registry;
    }
    let mut next = HandlerRegistry::default();
    for id in registry.protocols() {
        if let Some(handler) = registry.get(&id) {
            next.register(handler);
        }
    }
    next.register(Arc::new(PingHandler));
    Arc::new(next)
}

/// 对在册连接做一次探测往返：开流 → 协议握手 → 写 nonce → 回读比对。
/// 任何失败都带原因返回（观测用，禁止静默）；整体受 timeout 约束。
pub(crate) async fn probe_once(mux: &Mux, timeout: Duration) -> Result<(), String> {
    let id = ping_id();
    let call = async {
        let raw = mux
            .open_stream()
            .await
            .map_err(|e| format!("open stream: {e}"))?;
        let mut stream = open_with_protocol(raw, &id)
            .await
            .map_err(|e| format!("protocol handshake: {e}"))?;
        let nonce = nonce_bytes();
        write_frame(&mut stream, &nonce)
            .await
            .map_err(|e| format!("write nonce: {e}"))?;
        let reply = read_frame(&mut stream)
            .await
            .map_err(|e| format!("read reply: {e}"))?;
        if reply != nonce {
            return Err(format!("echo mismatch: {} bytes back", reply.len()));
        }
        Ok(())
    };
    match tokio::time::timeout(timeout, call).await {
        Ok(result) => result,
        Err(_) => Err(format!("probe timed out after {timeout:?}")),
    }
}

/// 8 字节 nonce：墙钟纳秒。仅用于回声比对，非安全用途。
fn nonce_bytes() -> Vec<u8> {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    nanos.to_le_bytes()[..8].to_vec()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ping_protocol_id_matches_relay_constant() {
        assert_eq!(PING_PROTOCOL, "/p2p-base/ping/1");
        assert_eq!(ping_id().as_str(), p2p_relay::proto_ids::PING);
    }

    #[test]
    fn registry_with_ping_keeps_user_handler() {
        struct User;
        #[async_trait::async_trait]
        impl ProtocolHandler for User {
            fn protocol(&self) -> ProtocolId {
                ping_id()
            }
            async fn handle(&self, _stream: BoxedStream) -> std::io::Result<()> {
                Ok(())
            }
        }
        let mut registry = HandlerRegistry::default();
        let user: Arc<dyn ProtocolHandler> = Arc::new(User);
        registry.register(user.clone());
        let merged = registry_with_ping(Arc::new(registry));
        let ptr_eq = Arc::ptr_eq(&merged.get(&ping_id()).expect("handler"), &user);
        assert!(ptr_eq, "user handler must not be overridden");
    }

    #[test]
    fn registry_with_ping_injects_when_absent() {
        let merged = registry_with_ping(Arc::new(HandlerRegistry::default()));
        assert!(merged.get(&ping_id()).is_some());
    }

    #[test]
    fn nonce_bytes_are_eight() {
        assert_eq!(nonce_bytes().len(), 8);
    }
}

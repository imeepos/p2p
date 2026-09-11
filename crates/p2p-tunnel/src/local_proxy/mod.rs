//! 访侧本地回环反代核心（契约 §3.3 / gui-contract §19.3）：实现填埋（W-TB）。
//! 迁移自 apps/gui/src-tauri/src/tunnel/{proxy,head,pump}.rs，行为零变化。
//! 只绑 `127.0.0.1` 字面量；一条浏览器连接 ↔ 一条隧道流；每请求重写 Host 与
//! 同面 Origin/Referer；响应与 body 流式转发禁整包缓冲。停止 = abort serve
//! 任务（listener 无部分状态；活动连接自然排空）。

mod conn;
mod session;

pub mod head;
pub mod pump;

#[cfg(test)]
mod tests;

use std::net::{Ipv4Addr, SocketAddr};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;

use tokio::net::TcpListener;

use crate::audit::TunnelAuditRecord;
use crate::{TunnelError, TunnelIo};

use session::SessionLog;

/// 开隧道缝：产出 ack 后的裸字节流（生产 = [crate::TunnelClient] 包装；
/// 单测 = 自足握手的直连裸流）。独立 trait 使反代测试免于耦合 TunnelClient 内部。
///
/// 出处：apps/gui/src-tauri/src/tunnel/proxy.rs:24-27（现形状逐字提炼）；
/// 数据面语义 specs/tunnel.md §3.1-§3.3。
#[async_trait::async_trait]
pub trait TunnelOpener: Send + Sync {
    /// `uid` = 16 hex 会话 id（两侧日志同源，§5.2）；`target` = `127.0.0.1:<port>`
    /// 字面量；成功返回 ack 后的 [TunnelIo]，拒绝/故障返回 [TunnelError]
    /// （错误码闭集 §4）。
    async fn open(&self, uid: &str, target: &str) -> Result<TunnelIo, TunnelError>;
}

/// 反代共享上下文（会话审计以 crate 内 [SessionLog] 承载，快照元素 =
/// [TunnelAuditRecord]（§5.2 八字段闭集，snake_case）；camelCase 映射留在
/// 产品装配层）。
pub struct ProxyCtx {
    target_port: u16,
    peer_id: String,
    opener: Arc<dyn TunnelOpener>,
    conns: AtomicU32,
    audit: SessionLog,
}

impl ProxyCtx {
    /// 活动连接数（tunnel_status 观测面）。
    /// 出处：apps/gui/src-tauri/src/tunnel/proxy.rs:39-41。
    pub fn active_conns(&self) -> u32 {
        self.conns.load(Ordering::Relaxed)
    }

    /// 审计快照（tunnel_status 的 sessions 字段，§5.2 八字段闭集）。
    /// 出处：apps/gui/src-tauri/src/tunnel/proxy.rs:43-47（async 形状保留，
    /// 会话账为异步锁账）。存续期会话：ended_at=0（§5.2「ended_at 为空」）。
    pub async fn audit_snapshot(&self) -> Vec<TunnelAuditRecord> {
        self.audit.snapshot().await
    }
}

/// 本地反代监听体。`bind` 只绑 127.0.0.1 字面量；`serve` 由调用方 spawn，
/// 停止 = abort 该任务（listener 无部分状态；活动连接自然排空）。
/// 出处：apps/gui/src-tauri/src/tunnel/proxy.rs:49-102；绑定面 §19.3-1 / §3.3。
pub struct LocalProxy {
    listener: TcpListener,
    pub ctx: Arc<ProxyCtx>,
}

impl LocalProxy {
    /// 绑定 127.0.0.1:0（§19.3-1 / §3.3：禁 localhost/0.0.0.0/::1，port 0 由
    /// OS 分配）；`target_port` = 被访目标端口（target 字面量由此拼装）；
    /// `peer_id` = 被访节点 PeerId（审计字段）；`opener` = 隧道开启缝。
    /// 出处：apps/gui/src-tauri/src/tunnel/proxy.rs:57-74。
    pub async fn bind(
        target_port: u16,
        peer_id: String,
        opener: Arc<dyn TunnelOpener>,
    ) -> std::io::Result<Self> {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await?;
        Ok(Self {
            listener,
            ctx: Arc::new(ProxyCtx::new(
                target_port,
                peer_id,
                opener,
                SessionLog::default(),
            )),
        })
    }

    /// 反代监听地址（bind 成功后必有值）。
    /// 出处：apps/gui/src-tauri/src/tunnel/proxy.rs:76-78。
    pub fn local_addr(&self) -> SocketAddr {
        self.listener.local_addr().expect("bound listener has addr")
    }

    /// accept 循环；非回环来源直接拒（防绑定面意外暴露）。
    /// 出处：apps/gui/src-tauri/src/tunnel/proxy.rs:80-101。
    pub async fn serve(self) {
        loop {
            match self.listener.accept().await {
                Ok((tcp, peer)) if peer.ip().is_loopback() => {
                    let ctx = Arc::clone(&self.ctx);
                    ctx.conns_add();
                    tokio::spawn(async move {
                        if let Err(reason) = conn::serve_conn(tcp, &ctx).await {
                            tracing::warn!(%reason, "tunnel proxy 连接处理失败");
                        }
                        ctx.conns_sub();
                    });
                }
                Ok((_, peer)) => tracing::warn!(%peer, "tunnel proxy 拒绝非回环来源"),
                Err(err) => {
                    tracing::warn!(error = %err, "tunnel proxy accept 失败");
                    tokio::time::sleep(Duration::from_millis(200)).await;
                }
            }
        }
    }
}

impl ProxyCtx {
    /// 桩期 [crate::audit::TunnelAudit] 字段换型为 [SessionLog]（存续期可见的
    /// 活账，GUI AuditLog 平移；TA 桩注记「随实现落地一并转正」即此）。
    fn new(
        target_port: u16,
        peer_id: String,
        opener: Arc<dyn TunnelOpener>,
        audit: SessionLog,
    ) -> Self {
        Self {
            target_port,
            peer_id,
            opener,
            conns: AtomicU32::new(0),
            audit,
        }
    }

    fn conns_add(&self) {
        self.conns.fetch_add(1, Ordering::Relaxed);
    }

    fn conns_sub(&self) {
        self.conns.fetch_sub(1, Ordering::Relaxed);
    }
}

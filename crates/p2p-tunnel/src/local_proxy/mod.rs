//! 访侧本地回环反代核心（契约 §3.3 / gui-contract §19.3）：签名桩（W-TA 契约先行）。
//! 实现填埋 = W-TB（迁移自 apps/gui/src-tauri/src/tunnel/{proxy,head,pump}.rs，
//! GUI 切换同卡）。只绑 `127.0.0.1` 字面量；一条浏览器连接 ↔ 一条隧道流；
//! 每请求重写 Host 与同面 Origin/Referer；响应与 body 流式转发禁整包缓冲。
//! 停止 = abort serve 任务（listener 无部分状态；活动连接自然排空）。

pub mod head;
pub mod pump;

use std::net::SocketAddr;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

use crate::audit::{TunnelAudit, TunnelAuditRecord};
use crate::{TunnelError, TunnelIo};

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

/// 反代共享上下文（GUI 装配的 `AuditLog`/`TunnelSessionAudit` 以 crate 内
/// [TunnelAudit]/[TunnelAuditRecord] 承载，camelCase 映射留在产品装配层）。
#[allow(dead_code)] // W-TB 填实现后随实现消费移除
pub struct ProxyCtx {
    target_port: u16,
    peer_id: String,
    opener: Arc<dyn TunnelOpener>,
    conns: AtomicU32,
    audit: TunnelAudit,
}

impl ProxyCtx {
    /// 活动连接数（tunnel_status 观测面）。
    /// 出处：apps/gui/src-tauri/src/tunnel/proxy.rs:39-41。
    pub fn active_conns(&self) -> u32 {
        todo!("W-TB")
    }

    /// 审计快照（tunnel_status 的 sessions 字段，§5.2 八字段闭集）。
    /// 出处：apps/gui/src-tauri/src/tunnel/proxy.rs:43-47（async 形状保留，
    /// 便于 TB 以现有异步审计账平移）。
    pub async fn audit_snapshot(&self) -> Vec<TunnelAuditRecord> {
        todo!("W-TB")
    }
}

/// 本地反代监听体。`bind` 只绑 127.0.0.1 字面量；`serve` 由调用方 spawn，
/// 停止 = abort 该任务（listener 无部分状态；活动连接自然排空）。
/// 出处：apps/gui/src-tauri/src/tunnel/proxy.rs:49-102；绑定面 §19.3-1 / §3.3。
#[allow(dead_code)] // W-TB 填实现后随实现消费移除
pub struct LocalProxy {
    listener: tokio::net::TcpListener,
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
        let _ = (target_port, peer_id, opener);
        todo!("W-TB")
    }

    /// 反代监听地址（bind 成功后必有值）。
    /// 出处：apps/gui/src-tauri/src/tunnel/proxy.rs:76-78。
    pub fn local_addr(&self) -> SocketAddr {
        todo!("W-TB")
    }

    /// accept 循环；非回环来源直接拒（防绑定面意外暴露）。
    /// 出处：apps/gui/src-tauri/src/tunnel/proxy.rs:80-101。
    pub async fn serve(self) {
        todo!("W-TB")
    }
}

impl ProxyCtx {
    #[allow(dead_code)] // W-TB 构造器；随实现落地一并转正
    fn new(
        target_port: u16,
        peer_id: String,
        opener: Arc<dyn TunnelOpener>,
        audit: TunnelAudit,
    ) -> Self {
        Self {
            target_port,
            peer_id,
            opener,
            conns: AtomicU32::new(0),
            audit,
        }
    }

    #[allow(dead_code)] // W-TB 填实现后随实现消费移除
    fn conns_add(&self) {
        self.conns.fetch_add(1, Ordering::Relaxed);
    }

    #[allow(dead_code)] // W-TB 填实现后随实现消费移除
    fn conns_sub(&self) {
        self.conns.fetch_sub(1, Ordering::Relaxed);
    }

    #[allow(dead_code)] // W-TB 填实现后随实现消费移除
    fn audit(&self) -> &TunnelAudit {
        &self.audit
    }
}

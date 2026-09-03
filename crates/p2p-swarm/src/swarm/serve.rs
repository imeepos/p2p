//! 已建连接的收流服务（E8 自 dial.rs 迁出）：accept 分发循环与使用度记账。
//!
//! serve 任务只持组件不持 Swarm：生命周期与连接一致，关停或断链即随组件
//! 释放；spawn 入口在装配期取一次 &Swarm 克隆组件，之后与 Swarm 存亡解耦。

use std::sync::Arc;

use p2p_identity::PeerId;
use p2p_mux::BoxedStream;
use p2p_protocol::{dispatch_inbound, ProtocolError};
use tokio::sync::{broadcast, watch};

use super::lifecycle::{LifecycleHandle, LifecycleMsg};
use super::{Mux, RegistryCell, Swarm};
use crate::lifecycle::LifecycleEvent;
use crate::liveness::{LivenessBook, LivenessSource};
use crate::pool::{ConnKind, ConnectionPool};
use crate::usage::{unix_now, ConnUsage, InflightGuard};
use crate::{CloseReason, NodeEvent};

/// serve 任务的独立组件集：不持有 Swarm 本体，生命周期与连接一致。
struct ServeCtx {
    pool: Arc<ConnectionPool>,
    registry: RegistryCell,
    events: broadcast::Sender<NodeEvent>,
    shutdown: watch::Receiver<bool>,
    /// E6：断链回报监督者（Connected→BackingOff 并排定重连）。
    lifecycle: LifecycleHandle,
    /// E8：活跃度判定账本（中继电路断链喂 RelaySlot 死信号）。
    liveness: Arc<LivenessBook>,
    /// E8：本连接的使用记账（在途流豁免空闲回收的判据）。
    usage: Option<Arc<ConnUsage>>,
    /// E8：连接类别（直连/中继电路，决定断链是否进活跃度判定）。
    kind: ConnKind,
}

/// 装配期入口：克隆组件派发 serve 任务，&Swarm 不进入任务（存亡解耦）。
pub(super) fn spawn_serve(
    swarm: &Swarm,
    peer: PeerId,
    mux: Mux,
    usage: Option<Arc<ConnUsage>>,
    kind: ConnKind,
) {
    let ctx = ServeCtx {
        pool: swarm.pool.clone(),
        registry: swarm.registry.clone(),
        events: swarm.events.clone(),
        shutdown: swarm.shutdown_rx.clone(),
        lifecycle: swarm.lifecycle.clone(),
        liveness: swarm.liveness.clone(),
        usage,
        kind,
    };
    tokio::spawn(serve_connection(ctx, peer, mux));
}

/// 收流分发循环：连接关闭或关停即出池并发 PeerDisconnected（断开路径可见）。
async fn serve_connection(ctx: ServeCtx, peer: PeerId, mux: Mux) {
    let mut shutdown = ctx.shutdown;
    loop {
        tokio::select! {
            _ = shutdown.changed() => break,
            stream = mux.accept_stream() => match stream {
                Some(stream) => {
                    // E8：入站流是活跃度正信号（收包=对端在网，刷新 last_seen），
                    // 同时计入使用记账（touch + 在途守护，豁免空闲回收）。
                    if let Some(usage) = &ctx.usage {
                        usage.touch(unix_now());
                    }
                    ctx.liveness.note_alive(peer, LivenessSource::Connection, unix_now());
                    let guard = ctx.usage.as_ref().map(|u| u.enter());
                    tokio::spawn(dispatch_stream(
                        ctx.registry.clone(),
                        ctx.events.clone(),
                        guard,
                        peer,
                        stream,
                    ));
                }
                None => break,
            },
        }
    }
    // 仅当本连接仍在册才发断开事件：被顶替的旧连接退出时池里已是新连接，
    // 此刻发 PeerDisconnected 是谎报（GUI 会把活连接渲染成断开）。
    // 挂断/回收/关停均先出池，走不到这里，不会与本端归档重复。
    if ctx.pool.remove_if_same(&peer, &mux) {
        let _ = ctx.events.send(NodeEvent::PeerDisconnected { peer });
        // E8：非本端发起的断链归因 Error（对端消失/网络故障）
        let _ = ctx.lifecycle.events.send(LifecycleEvent::ConnectionClosed {
            peer,
            reason: CloseReason::Error,
        });
        // E6 钩子：本连接确已出池（被顶替的旧连接不进来，不谎报断链）
        ctx.lifecycle.notify(LifecycleMsg::LinkLost { peer });
        // E8：中继电路断链即 relay 槽失活（活跃度死信号）；直连断链是连接
        // 事实，由状态机全权处理，不重复进活跃度（见 liveness.rs 模块注释）。
        if ctx.kind == ConnKind::RelayCircuit {
            ctx.liveness
                .note_dead(peer, LivenessSource::RelaySlot, unix_now());
        }
    }
}

/// 单条入站流分发：协议违规（含未注册协议）发事件；纯 io 关闭只留调试日志。
/// 守护横跨整个分发过程：入站流在途期间该连接免于空闲回收。
async fn dispatch_stream(
    registry: RegistryCell,
    events: broadcast::Sender<NodeEvent>,
    guard: Option<InflightGuard>,
    peer: PeerId,
    stream: BoxedStream,
) {
    let _guard = guard;
    let snapshot = registry.lock().expect("registry lock").clone();
    match dispatch_inbound(stream, &snapshot).await {
        Ok(()) => {}
        Err(ProtocolError::Io(err)) => {
            tracing::debug!(%peer, error = %err, "inbound stream closed");
        }
        Err(other) => {
            let reason = other.to_string();
            tracing::warn!(%peer, %reason, "protocol violation on inbound stream");
            let _ = events.send(NodeEvent::ProtocolViolation { peer, reason });
        }
    }
}

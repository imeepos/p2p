//! 空闲连接回收（E8，调研建议 4）：周期扫描连接池，回收空闲超过阈值
//! 且无在途业务流的连接。回收动作发关闭原因事件（Idle 档）并出册生命
//! 周期监督（不自动重连——回收不是故障，重拨是按需行为）。
//!
//! 使用中豁免：有在途业务流的连接绝不回收，判据见 ConnUsage::is_idle。
//! 探活流量不计入使用（否则监督者的周期探测让回收永不触发），见 usage.rs。

use std::sync::{Arc, Weak};
use std::time::Duration;

use tokio::time::MissedTickBehavior;

use super::lifecycle::LifecycleMsg;
use super::Swarm;
use crate::lifecycle::LifecycleEvent;
use crate::usage::unix_now;
use crate::{CloseReason, NodeEvent};

/// 空闲回收参数（E8）。全部可配；默认值依据见 Default 实现。
#[derive(Clone, Debug)]
pub struct ReclaimConfig {
    /// false 时不启动回收任务，行为与 E7 之前一致。
    pub enabled: bool,
    /// 空闲阈值：无在途流且距最后业务使用超过该时长的连接可回收。
    pub idle_threshold: Duration,
    /// 扫描周期。
    pub scan_interval: Duration,
}

impl Default for ReclaimConfig {
    fn default() -> Self {
        Self {
            // 调研建议 4 默认采纳；显式 false 可回到无回收行为
            enabled: true,
            // libp2p 默认 10s 面向即时复用场景；本栈上层（GUI 邻居表/CLI）
            // 语义是对端常驻可见，过激回收诱发重拨风暴。120s 比探活周期
            // （10s）高一个量级，死重连接至多滞留约阈值+一个扫描周期。
            idle_threshold: Duration::from_secs(120),
            // 阈值的 1/4：回收延迟上界 = threshold + interval，扫描开销
            // O(池大小)，更密只添空转。
            scan_interval: Duration::from_secs(30),
        }
    }
}

/// 装配期入口：enabled 时启动回收任务（Swarm 装配完成后调用）。
pub(super) fn spawn_reclaim(swarm: &Arc<Swarm>, cfg: &ReclaimConfig) {
    if !cfg.enabled {
        tracing::info!("idle reclaim disabled; reclaimer not started");
        return;
    }
    tokio::spawn(reclaim_loop(Arc::downgrade(swarm), cfg.clone()));
    tracing::info!(
        idle_threshold = ?cfg.idle_threshold,
        scan_interval = ?cfg.scan_interval,
        "idle connection reclaimer started"
    );
}

async fn reclaim_loop(swarm: Weak<Swarm>, cfg: ReclaimConfig) {
    let mut shutdown = match swarm.upgrade() {
        Some(strong) => strong.shutdown_rx.clone(),
        None => return,
    };
    let mut ticker = tokio::time::interval(cfg.scan_interval);
    ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);
    loop {
        tokio::select! {
            _ = shutdown.changed() => break,
            _ = ticker.tick() => {}
        }
        let Some(strong) = swarm.upgrade() else { break };
        reclaim_once(&strong, &cfg);
    }
}

/// 单轮回收：出池→关闭→断开事件→原因归档→出册监督者（不自动重连）。
fn reclaim_once(swarm: &Swarm, cfg: &ReclaimConfig) {
    let reclaimed = swarm.pool.reclaim_idle(cfg.idle_threshold, unix_now());
    for (peer, mux) in reclaimed {
        tracing::info!(%peer, idle = ?cfg.idle_threshold, "reclaiming idle connection");
        // 出池先于关闭：serve 循环退出时 remove_if_same 不中，不重复发断开
        mux.close();
        swarm.emit(NodeEvent::PeerDisconnected { peer });
        let _ = swarm
            .lifecycle
            .events
            .send(LifecycleEvent::ConnectionClosed {
                peer,
                reason: CloseReason::Idle,
            });
        // 挂断语义出册：回收不是故障，不进 BackingOff/自动重连
        swarm.lifecycle.notify(LifecycleMsg::HungUp { peer });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_matches_documented_rationale() {
        let cfg = ReclaimConfig::default();
        assert!(cfg.enabled);
        assert_eq!(cfg.idle_threshold, Duration::from_secs(120));
        assert_eq!(cfg.scan_interval, Duration::from_secs(30));
    }
}

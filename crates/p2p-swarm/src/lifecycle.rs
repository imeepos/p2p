//! 对端连接生命周期契约（E6）：每 PeerId 一份显式状态机与可配置参数。
//!
//! 状态机是事实模型：只记录「连接是否在册、拨号是否在途、是否在退避重试」，
//! 拒绝非事实的转移（Connected→Connecting、自转移等）并留 warn 日志。
//! 四态语义：
//! - Disconnected：无在册连接，也无在途拨号
//! - Connecting：一次拨号在途（首拨或重连重试）
//! - Connected：池内有在册连接
//! - BackingOff：已知不可达，等待下一次重连（指数退避 + 抖动）
//!
//! 合法转移表（peer_machine_tests 全覆盖，非法一律拒绝）：
//! Disconnected→Connecting/Connected；Connecting→{Connected,BackingOff,Disconnected}；
//! Connected→{BackingOff,Disconnected}；BackingOff→{Connecting,Connected,Disconnected}。
//!
//! 默认值依据（PeerLifecycleConfig::default，逐项）：
//! - probe_interval 10s：局域网/跨网 RTT 均毫秒级，更密只添无谓流量；
//!   与常见心跳区间（5-30s）对齐取中位。
//! - probe_timeout 3s：E4 实测中继电路建立 ~90ms 量级，3s 为 30 倍余量，
//!   覆盖开流 + 协议握手 + 帧往返的最坏路径。
//! - max_probe_misses 3：连续 3 次（约 30s 无响应）才判离线，容忍单次
//!   网络抖动与对端瞬时停顿，避免误判翻脸。
//! - reconnect_base 1s / reconnect_max 60s：首次重连快速尝试；上限 1 分钟
//!   内持续重试但不密集打爆对端与网络（libp2p 常用量级）。
//! - reconnect_jitter 0.2：±20% 抖动，防多节点同时重连同一对端（惊群）。
//! - reset_min_uptime 30s：会话存活满 30s 视为健康（E5 语义：闪断不复位）；
//!   高于秒级抖动（wifi 切换），低于典型故障间隔。

use std::time::Duration;

use p2p_identity::PeerId;

/// 对端连接状态（E6）。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConnState {
    Disconnected,
    Connecting,
    Connected,
    BackingOff,
}

impl ConnState {
    pub fn as_str(self) -> &'static str {
        match self {
            ConnState::Disconnected => "disconnected",
            ConnState::Connecting => "connecting",
            ConnState::Connected => "connected",
            ConnState::BackingOff => "backing_off",
        }
    }
}

/// 对端生命周期事件（E6）：走独立广播通道（Swarm::subscribe_lifecycle）。
///
/// 机制选择说明（任务书允许「NodeEvent 加法变体或等价事件机制」）：
/// 不动 NodeEvent——既有事件流被多处白名单式严格消费（如 itest hairpin
/// 用例只认单一事件），加法变体对它们是行为扰动；独立通道让冻结事件流
/// 零扰动，生命周期事件获得独立订阅语义与独立容量，可观测性反而更强。
#[derive(Clone, Debug)]
pub enum LifecycleEvent {
    /// 状态机转移（每 PeerId 一份，状态见 [ConnState]）。
    PeerStateChanged {
        peer: PeerId,
        from: ConnState,
        to: ConnState,
    },
    /// 探活判定离线（连续探测未命中后关死半开连接）。
    /// 与传输层 PeerDisconnected 区分：本事件代表「主动探活确认对端失联」。
    PeerDown { peer: PeerId, reason: String },
    /// 对端恢复在线——有过下线史的对端再次建成连接
    /// （重连成功或其主动拨入），与 PeerDown/PeerDisconnected 成对。
    PeerUp { peer: PeerId },
    /// E8（加法）：连接关闭原因归档（idle/error/refused/local 四档，见
    /// [crate::CloseReason]）。与 NodeEvent::PeerDisconnected 成对出现：
    /// 前者归因，后者只宣告断开事实。
    ConnectionClosed {
        peer: PeerId,
        reason: crate::CloseReason,
    },
    /// E8（加法）：统一活跃度判定（多源合并去重后的唯一活跃度事件，
    /// 判定语义与状态机关系见 [crate::liveness] 模块注释）。
    PeerLiveness(crate::PeerLiveness),
}

/// 非法转移：拒绝时保留 from/to 现场供日志与事件使用。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TransitionError {
    pub from: ConnState,
    pub to: ConnState,
}

impl std::fmt::Display for TransitionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "illegal transition {} -> {}",
            self.from.as_str(),
            self.to.as_str()
        )
    }
}

/// 每 PeerId 一份的连接状态机：transition 是唯一状态入口。
#[derive(Debug)]
pub struct PeerMachine {
    state: ConnState,
}

impl PeerMachine {
    pub fn new() -> Self {
        Self {
            state: ConnState::Disconnected,
        }
    }

    pub fn state(&self) -> ConnState {
        self.state
    }

    /// 状态转移：合法返回前一状态（供 from→to 事件），非法拒绝且不改状态。
    pub fn transition(&mut self, to: ConnState) -> Result<ConnState, TransitionError> {
        if !is_legal(self.state, to) {
            return Err(TransitionError {
                from: self.state,
                to,
            });
        }
        Ok(std::mem::replace(&mut self.state, to))
    }
}

impl Default for PeerMachine {
    fn default() -> Self {
        Self::new()
    }
}

/// 事实约束：连接建成（→Connected）与断开（Connected→…）由传输事实驱动，
/// 无会话何来退避（Disconnected→BackingOff 非法），已连接无需再拨（Connected→Connecting 非法）。
fn is_legal(from: ConnState, to: ConnState) -> bool {
    use ConnState::*;
    matches!(
        (from, to),
        (Disconnected, Connecting)
            | (Disconnected, Connected)
            | (Connecting, Connected)
            | (Connecting, BackingOff)
            | (Connecting, Disconnected)
            | (Connected, BackingOff)
            | (Connected, Disconnected)
            | (BackingOff, Connecting)
            | (BackingOff, Connected)
            | (BackingOff, Disconnected)
    )
}

/// 生命周期参数（E6）。全部可配；默认值依据见模块注释。
#[derive(Clone, Debug)]
pub struct PeerLifecycleConfig {
    /// false 时监督者不启动，行为与 E6 之前一致。
    pub enabled: bool,
    /// Connected 对端的探活周期。
    pub probe_interval: Duration,
    /// 单次探活往返预算（含开流与协议握手）。
    pub probe_timeout: Duration,
    /// 连续未命中该次数即判离线（PeerDown）。
    pub max_probe_misses: u32,
    /// 重连退避基数（首次重连等待）。
    pub reconnect_base: Duration,
    /// 重连退避上限。
    pub reconnect_max: Duration,
    /// ± 比例抖动（0.0-1.0）；0 关闭（测试确定性）。
    pub reconnect_jitter: f64,
    /// 会话存活满该时长才算健康；健康会话后的重连成功才复位退避（E5 语义）。
    pub reset_min_uptime: Duration,
}

impl Default for PeerLifecycleConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            probe_interval: Duration::from_secs(10),
            probe_timeout: Duration::from_secs(3),
            max_probe_misses: 3,
            reconnect_base: Duration::from_secs(1),
            reconnect_max: Duration::from_secs(60),
            reconnect_jitter: 0.2,
            reset_min_uptime: Duration::from_secs(30),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 按合法路径把机器驱动到目标态（Disconnected→Connecting→…）。
    fn machine_at(state: ConnState) -> PeerMachine {
        let mut m = PeerMachine::new();
        if state != ConnState::Disconnected {
            m.transition(ConnState::Connecting)
                .expect("setup: Disconnected -> Connecting");
        }
        match state {
            ConnState::Connected => {
                m.transition(ConnState::Connected)
                    .expect("setup: -> Connected");
            }
            ConnState::BackingOff => {
                m.transition(ConnState::BackingOff)
                    .expect("setup: -> BackingOff");
            }
            _ => {}
        }
        m
    }

    #[test]
    fn legal_transitions_accepted_and_report_previous() {
        let cases = [
            (ConnState::Disconnected, ConnState::Connecting),
            (ConnState::Disconnected, ConnState::Connected),
            (ConnState::Connecting, ConnState::Connected),
            (ConnState::Connecting, ConnState::BackingOff),
            (ConnState::Connecting, ConnState::Disconnected),
            (ConnState::Connected, ConnState::BackingOff),
            (ConnState::Connected, ConnState::Disconnected),
            (ConnState::BackingOff, ConnState::Connecting),
            (ConnState::BackingOff, ConnState::Connected),
            (ConnState::BackingOff, ConnState::Disconnected),
        ];
        for (from, to) in cases {
            let mut m = machine_at(from);
            assert_eq!(
                m.transition(to).expect("legal transition"),
                from,
                "{from:?}->{to:?}"
            );
            assert_eq!(m.state(), to);
        }
    }

    #[test]
    fn illegal_transitions_rejected_without_mutation() {
        let cases = [
            (ConnState::Disconnected, ConnState::BackingOff),
            (ConnState::Disconnected, ConnState::Disconnected),
            (ConnState::Connecting, ConnState::Connecting),
            (ConnState::Connected, ConnState::Connecting),
            (ConnState::Connected, ConnState::Connected),
            (ConnState::BackingOff, ConnState::BackingOff),
        ];
        for (from, to) in cases {
            let mut m = machine_at(from);
            let err = m.transition(to).expect_err("must reject");
            assert_eq!(err, TransitionError { from, to });
            assert_eq!(m.state(), from, "rejected transition must not mutate");
        }
    }

    #[test]
    fn config_defaults_match_documented_rationale() {
        let cfg = PeerLifecycleConfig::default();
        assert!(cfg.enabled);
        assert_eq!(cfg.probe_interval, Duration::from_secs(10));
        assert_eq!(cfg.probe_timeout, Duration::from_secs(3));
        assert_eq!(cfg.max_probe_misses, 3);
        assert_eq!(cfg.reconnect_base, Duration::from_secs(1));
        assert_eq!(cfg.reconnect_max, Duration::from_secs(60));
        assert!((cfg.reconnect_jitter - 0.2).abs() < f64::EPSILON);
        assert_eq!(cfg.reset_min_uptime, Duration::from_secs(30));
    }
}

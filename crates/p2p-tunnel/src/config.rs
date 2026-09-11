//! 被访侧准入（契约 §5）：目标白名单显式配置（精确 `127.0.0.1:<port>`，默认空
//! = 全拒）、按次开启（会话态默认关闭）、并发上限与超时参数。
//! 开关/白名单为 Rust 内部 API（GUI 装配与 itest 同源消费）。

use std::collections::HashSet;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use tokio::sync::{OwnedSemaphorePermit, Semaphore};

use crate::wire::{is_loopback_literal_target, TunnelErrorCode, TS_WINDOW_SECS};

/// 被访侧参数。allowlist 默认空、开关默认关（双默认 = 全拒）。
#[derive(Debug, Clone)]
pub struct TunnelServeConfig {
    pub allowlist: HashSet<String>,
    pub max_concurrent: usize,
    /// 首帧票据超时（契约 §1：5s 内未收到合法票据即拒）。
    pub ticket_timeout: Duration,
    /// 票据 ts 允许窗口（秒）。
    pub ts_window_secs: u64,
    /// 出站分块载荷上限（契约 §1：≤64 KiB）。
    pub chunk_size: usize,
    /// 整隧道生命周期上限；None = 交由底座传输层超时收敛。
    pub session_timeout: Option<Duration>,
}

impl Default for TunnelServeConfig {
    fn default() -> Self {
        Self {
            allowlist: HashSet::new(),
            max_concurrent: 4,
            ticket_timeout: Duration::from_secs(5),
            ts_window_secs: TS_WINDOW_SECS,
            chunk_size: 64 * 1024,
            session_timeout: None,
        }
    }
}

/// 并发许可：持有期 = 隧道会话期，drop 即归还（内容无需读取，仅作存活凭据）。
pub struct TunnelPermit(#[allow(dead_code)] OwnedSemaphorePermit);

/// 闸门会话态快照（GUI `TunnelServeStatus` 的数据源，camelCase 序列化在 GUI 层）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GateStatus {
    pub enabled: bool,
    /// 精确 `127.0.0.1:<port>` 白名单（字典序）。
    pub allow: Vec<String>,
    /// 在途会话数（许可被持有中）。
    pub active_sessions: usize,
}

/// 会话态闸门：开关（默认关）+ 白名单 + 并发许可。Clone 共享同一状态。
#[derive(Clone)]
pub struct TunnelGate {
    state: Arc<Mutex<GateState>>,
    permits: Arc<Semaphore>,
}

struct GateState {
    enabled: bool,
    allowlist: HashSet<String>,
    cfg: Arc<TunnelServeConfig>,
}

impl TunnelGate {
    pub fn new(cfg: TunnelServeConfig) -> Self {
        let permits = Arc::new(Semaphore::new(cfg.max_concurrent.max(1)));
        let allowlist: HashSet<String> = cfg
            .allowlist
            .iter()
            .filter(|t| is_loopback_literal_target(t))
            .cloned()
            .collect();
        if allowlist.len() != cfg.allowlist.len() {
            tracing::warn!("allowlist 含非回环字面量形态条目，已丢弃");
        }
        Self {
            state: Arc::new(Mutex::new(GateState {
                enabled: false,
                allowlist,
                cfg: Arc::new(cfg),
            })),
            permits,
        }
    }

    fn states(&self) -> MutexGuard<'_, GateState> {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// 静态参数快照（超时/分块等）。
    pub fn cfg(&self) -> Arc<TunnelServeConfig> {
        self.states().cfg.clone()
    }

    /// 按次开关（会话态，默认关）。
    pub fn set_enabled(&self, on: bool) {
        self.states().enabled = on;
        tracing::info!(enabled = on, "tunnel serve switch");
    }

    /// 白名单整组替换（非法形态条目丢弃并留痕）。
    pub fn set_allowlist(&self, targets: HashSet<String>) {
        let mut state = self.states();
        state.allowlist = targets
            .iter()
            .filter(|t| is_loopback_literal_target(t))
            .cloned()
            .collect();
        tracing::info!(count = state.allowlist.len(), "tunnel allowlist updated");
    }

    pub fn enabled(&self) -> bool {
        self.states().enabled
    }

    /// 单目标加入白名单（累积；先校验后动作，非法即 Err 不部分生效）。
    pub fn add_allow(&self, target: &str) -> Result<(), String> {
        if !is_loopback_literal_target(target) {
            return Err(format!(
                "target 须为 127.0.0.1:<port> 字面量（端口 1-65535），当前: {target}"
            ));
        }
        self.states().allowlist.insert(target.to_string());
        Ok(())
    }

    /// 会话态快照（开关/白名单/活跃会话数）。
    pub fn status(&self) -> GateStatus {
        let state = self.states();
        let mut allow: Vec<String> = state.allowlist.iter().cloned().collect();
        allow.sort();
        GateStatus {
            enabled: state.enabled,
            allow,
            active_sessions: state.cfg.max_concurrent.max(1) - self.permits.available_permits(),
        }
    }

    /// 准入三连判（fail-closed）：开关 → 白名单精确匹配 → 并发许可。
    pub fn authorize(&self, target: &str) -> Result<TunnelPermit, TunnelErrorCode> {
        let state = self.states();
        if !state.enabled {
            return Err(TunnelErrorCode::Shutdown);
        }
        if !state.allowlist.contains(target) {
            return Err(TunnelErrorCode::TargetNotAllowed);
        }
        drop(state);
        self.permits
            .clone()
            .try_acquire_owned()
            .map(TunnelPermit)
            .map_err(|_| TunnelErrorCode::Busy)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gate() -> TunnelGate {
        TunnelGate::new(TunnelServeConfig {
            allowlist: HashSet::from(["127.0.0.1:8014".to_string()]),
            max_concurrent: 1,
            ..Default::default()
        })
    }

    #[test]
    fn default_gate_is_off_and_default_config_denies_all() {
        let off = TunnelGate::new(TunnelServeConfig::default());
        assert!(!off.enabled());
        assert!(matches!(
            off.authorize("127.0.0.1:80"),
            Err(TunnelErrorCode::Shutdown)
        ));
        assert!(
            TunnelServeConfig::default().allowlist.is_empty(),
            "默认空白名单 = 全拒"
        );
    }

    #[test]
    fn enabled_gate_enforces_exact_allowlist() {
        let gate = gate();
        gate.set_allowlist(HashSet::from([
            "127.0.0.1:8014".to_string(),
            "localhost:80".to_string(),
        ]));
        gate.set_enabled(true);
        assert!(gate.authorize("127.0.0.1:8014").is_ok());
        assert!(matches!(
            gate.authorize("127.0.0.1:9"),
            Err(TunnelErrorCode::TargetNotAllowed)
        ));
        assert!(matches!(
            gate.authorize("127.0.0.1:8014 "),
            Err(TunnelErrorCode::TargetNotAllowed)
        ));
    }

    #[tokio::test]
    async fn concurrency_cap_yields_busy_then_release() {
        let gate = gate();
        gate.set_enabled(true);
        let permit = gate.authorize("127.0.0.1:8014").unwrap();
        assert!(matches!(
            gate.authorize("127.0.0.1:8014"),
            Err(TunnelErrorCode::Busy)
        ));
        drop(permit);
        assert!(gate.authorize("127.0.0.1:8014").is_ok(), "许可归还后可再入");
    }

    #[test]
    fn non_loopback_allowlist_entries_are_dropped() {
        let gate = TunnelGate::new(TunnelServeConfig {
            allowlist: HashSet::from([
                "localhost:80".to_string(),
                "127.0.0.1:81".to_string(),
            ]),
            ..Default::default()
        });
        gate.set_enabled(true);
        assert!(matches!(
            gate.authorize("localhost:80"),
            Err(TunnelErrorCode::TargetNotAllowed)
        ));
        assert!(gate.authorize("127.0.0.1:81").is_ok());
    }

    #[test]
    fn add_allow_validates_first_then_accumulates_and_status_counts() {
        let gate = gate();
        assert!(gate.add_allow("localhost:80").is_err(), "非法目标被拒");
        assert!(!gate.enabled(), "校验失败不得部分生效");
        assert!(gate.add_allow("127.0.0.1:8014").is_ok());
        gate.set_enabled(true);
        gate.add_allow("127.0.0.1:8015").unwrap();
        let status = gate.status();
        assert_eq!(status.allow, vec!["127.0.0.1:8014", "127.0.0.1:8015"]);
        assert!(status.enabled);
        assert_eq!(status.active_sessions, 0);
        let permit = gate.authorize("127.0.0.1:8014").unwrap();
        assert_eq!(gate.status().active_sessions, 1);
        drop(permit);
        assert_eq!(gate.status().active_sessions, 0);
    }
}

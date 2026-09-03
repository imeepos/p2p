//! 中继候选排序（负载感知选择）：CSPF 剪枝语义 + widest-shortest 字典序。
//!
//! 满载候选（达到拒绝线）沉底但保留在序列里兜底——服务端准入是硬拒绝
//! （GLOBAL_CAPACITY），排序只是软引导，两层合流（TURN 486 语义 +
//! 客户端多候选回退）。滞回防震荡、抖动防羊群；陈旧快照即可驱动选择
//! （power-of-two-choices 结论）。

use std::time::{SystemTime, UNIX_EPOCH};

use p2p_relay::RelayHealthSnapshot;

/// 负载分档粒度（permille）：主关键字按档比较，避免细粒度水位来回翻转。
const LOAD_BUCKET_PERMILLE: u32 = 100;

/// 选择参数（门槛型，默认值见 [RelaySelectionCfg::default]）。
#[derive(Debug, Clone)]
pub(super) struct RelaySelectionCfg {
    /// 负载拒绝线（permille）：达到该值的候选沉底兜底，不再优先派发。
    pub load_reject_permille: u32,
    /// 滞回余量（ms）：现任与最优同负载档且分差未超该值时，现任保持优先。
    pub hysteresis_ms: u64,
    /// 抖动幅度（ms）：RTT 评分加 U[0, jitter] 随机量，打散并列防扎堆。
    pub jitter_ms: u64,
}

impl Default for RelaySelectionCfg {
    fn default() -> Self {
        Self {
            load_reject_permille: 900,
            hysteresis_ms: 100,
            jitter_ms: 20,
        }
    }
}

/// 一个候选的观测输入。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct RelayCandidate {
    /// 会话在注册序列中的下标。
    pub index: usize,
    pub health: RelayHealthSnapshot,
}

/// 依序派发下标：健康候选在前（负载档升序、RTT 决胜、抖动打散），
/// 满载候选沉底兜底；滞回生效时现任提到最前。
pub(super) fn order_candidates(
    cands: &[RelayCandidate],
    current: Option<usize>,
    cfg: &RelaySelectionCfg,
    mut jitter: impl FnMut() -> u64,
) -> Vec<usize> {
    let mut scored: Vec<(u32, u64, usize)> = cands
        .iter()
        .map(|c| {
            (
                c.health.load_permille.min(cfg.load_reject_permille) / LOAD_BUCKET_PERMILLE,
                c.health.rtt_ema_ms + jitter(),
                c.index,
            )
        })
        .collect();
    scored.sort_unstable_by_key(|s| (s.0, s.1));
    let mut order: Vec<usize> = scored.iter().map(|s| s.2).collect();

    if let Some(cur) = current {
        let cur_scored = scored.iter().find(|s| s.2 == cur);
        if let (Some(&(cl, cr, _)), Some(&(bl, br, _))) = (cur_scored, scored.first()) {
            let cur_load = cl * LOAD_BUCKET_PERMILLE;
            if cur_load < cfg.load_reject_permille
                && cl == bl
                && br.saturating_add(cfg.hysteresis_ms) >= cr
            {
                // 现任未掉档且分差在余量内：现任优先，其余保持排序（稳定排序）
                order.sort_by_key(|i| if *i == cur { 0u8 } else { 1u8 });
            }
        }
    }
    order
}

/// 进程内低成本抖动源：epoch 纳秒取模（并列打散用，非安全敏感）。
pub(super) fn jitter_value(jitter_ms: u64) -> u64 {
    if jitter_ms == 0 {
        return 0;
    }
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| u64::from(d.subsec_nanos()) % (jitter_ms + 1))
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg() -> RelaySelectionCfg {
        RelaySelectionCfg::default()
    }

    fn cand(index: usize, load: u32, rtt: u64) -> RelayCandidate {
        RelayCandidate {
            index,
            health: RelayHealthSnapshot {
                rtt_ema_ms: rtt,
                load_permille: load,
            },
        }
    }

    #[test]
    fn empty_candidates_order_empty() {
        assert!(order_candidates(&[], None, &cfg(), || 0).is_empty());
    }

    #[test]
    fn load_primary_rtt_secondary() {
        let c = [cand(0, 500, 10), cand(1, 100, 50), cand(2, 100, 20)];
        assert_eq!(order_candidates(&c, None, &cfg(), || 0), vec![2, 1, 0]);
    }

    #[test]
    fn saturated_sinks_but_stays_fallback() {
        let c = [cand(0, 950, 5), cand(1, 200, 999)];
        assert_eq!(order_candidates(&c, None, &cfg(), || 0), vec![1, 0]);
    }

    #[test]
    fn hysteresis_keeps_current_within_margin() {
        let c = [cand(0, 100, 50), cand(1, 100, 30)];
        // 现任分差 20ms < 100ms 余量：保持现任
        assert_eq!(order_candidates(&c, Some(0), &cfg(), || 0), vec![0, 1]);
    }

    #[test]
    fn current_loses_when_margin_exceeded() {
        let c = [cand(0, 100, 200), cand(1, 100, 30)];
        assert_eq!(order_candidates(&c, Some(0), &cfg(), || 0), vec![1, 0]);
    }

    #[test]
    fn rejected_current_not_preferred() {
        let c = [cand(0, 950, 5), cand(1, 100, 50)];
        assert_eq!(order_candidates(&c, Some(0), &cfg(), || 0), vec![1, 0]);
    }

    #[test]
    fn cross_bucket_margin_ignored() {
        // 不同负载档不适用滞回：轻载者直接胜出
        let c = [cand(0, 100, 10), cand(1, 300, 999)];
        assert_eq!(order_candidates(&c, Some(0), &cfg(), || 0), vec![0, 1]);
    }
}

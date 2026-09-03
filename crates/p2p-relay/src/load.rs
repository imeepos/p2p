//! 服务端负载水位（permille）：链路/电路/限速桶三资源占用率取最大。
//!
//! 语义对齐路径选择的瓶颈观：剩余服务能力由最弱资源决定，故取最大占用率
//! 而非平均。随 Reserved/KeepAliveAck 广播，供客户端负载感知选中继。

/// 三资源占用率取最大，permille（0..=1000）输出；上限为 0 视作未配置，计 0。
pub(crate) fn compute_load_permille(
    links_used: usize,
    max_links: usize,
    circuits_used: usize,
    max_circuits: usize,
    buckets_used: usize,
    max_buckets: usize,
) -> u32 {
    let frac = |used: usize, cap: usize| -> f64 {
        if cap == 0 {
            0.0
        } else {
            used as f64 / cap as f64
        }
    };
    let worst = frac(links_used, max_links)
        .max(frac(circuits_used, max_circuits))
        .max(frac(buckets_used, max_buckets));
    (worst * 1000.0).round().clamp(0.0, 1000.0) as u32
}

#[cfg(test)]
mod tests {
    use super::compute_load_permille as load;

    #[test]
    fn empty_relay_reports_zero() {
        assert_eq!(load(0, 256, 0, 1024, 0, 256), 0);
    }

    #[test]
    fn weakest_resource_dominates() {
        // 电路 1/2 = 500‰ 远高于链路 1/256 约 4‰
        assert_eq!(load(1, 256, 1, 2, 1, 256), 500);
    }

    #[test]
    fn saturated_resource_clamps_at_thousand() {
        assert_eq!(load(0, 256, 3, 2, 0, 256), 1000);
    }

    #[test]
    fn zero_cap_counts_as_unconfigured() {
        assert_eq!(load(0, 0, 0, 0, 0, 0), 0);
    }
}

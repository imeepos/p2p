//! rd 命令/装配面的配置缺省解析（CC2：rdRequireApproval/rdFps 装配消费）。
//! rd.rs 已超 300 行红线，新增逻辑居子模块（rd/relay.rs 先例）。

/// rd_host_start 审批闸取值：显式参数优先（运行态覆盖），None 回落配置缺省
/// （GuiConfig.rdRequireApproval，缺省 true 零行为变化）。
pub(crate) fn effective_require_approval(explicit: Option<bool>, configured: bool) -> bool {
    explicit.unwrap_or(configured)
}

/// rdFps 合法域 1..=60（契约 §3）；越界回缺省 15 并告警（不静默）。
pub(crate) fn sanitized_initial_fps(v: u8) -> u8 {
    if (1..=60).contains(&v) {
        return v;
    }
    tracing::warn!(value = v, "rdFps 越界，回缺省 15");
    15
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn approval_explicit_overrides_configured_default() {
        assert!(
            effective_require_approval(None, true),
            "None 回落配置缺省 true"
        );
        assert!(
            !effective_require_approval(None, false),
            "None 回落配置缺省 false"
        );
        assert!(
            !effective_require_approval(Some(false), true),
            "显式参数覆盖配置"
        );
        assert!(
            effective_require_approval(Some(true), false),
            "显式参数覆盖配置"
        );
    }

    #[test]
    fn initial_fps_in_domain_passes_through_and_out_of_domain_falls_back() {
        assert_eq!(sanitized_initial_fps(1), 1);
        assert_eq!(sanitized_initial_fps(30), 30);
        assert_eq!(sanitized_initial_fps(60), 60);
        assert_eq!(sanitized_initial_fps(0), 15, "越界下界回缺省并告警");
        assert_eq!(sanitized_initial_fps(61), 15, "越界上界回缺省并告警");
    }
}

//! 时钟注入（authz-role-design §7，沿 repair-enforce Clock 先例）：
//! granted_at 与判定时刻都经 trait 注入，测试用脚本化时钟确定性推进。

/// 注入时钟：返回 Unix 秒。
pub trait Clock {
    fn now_unix(&self) -> u64;
}

/// 生产时钟：系统 Unix 秒；时钟倒退（先于纪元）回落 0，不 panic
/// （对齐 p2p-cli/llm_share 与 acp 域授时口径）。
pub struct SystemClock;

impl Clock for SystemClock {
    fn now_unix(&self) -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0)
    }
}

/// 引用透明：借用的时钟同样可注入（测试里保留句柄做确定性推进）。
impl<C: Clock + ?Sized> Clock for &C {
    fn now_unix(&self) -> u64 {
        (**self).now_unix()
    }
}

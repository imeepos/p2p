//! 小工具：毫秒时间戳（事件盖戳与状态快照共用）。

use std::time::{SystemTime, UNIX_EPOCH};

/// 当前 Unix 毫秒时间戳；系统时钟早于纪元时返回 0（仅损失排序，不 panic）。
pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
        .unwrap_or(0)
}

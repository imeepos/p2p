//! host 运行时共享状态：质量协商结果 + 审批集（控制/视频处理器共用）。

use std::collections::HashSet;
use std::sync::{Arc, Mutex};

use p2p::PeerId;

/// 已协商质量档位（viewer 请求 → host 采纳）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QualityState {
    pub fps: u8,
    pub scale: u8,
    pub codec: u8,
}

impl Default for QualityState {
    fn default() -> Self {
        Self {
            fps: 15,
            scale: 100,
            codec: rd_wire::video::CODEC_RAW_RGBA,
        }
    }
}

/// host 运行时状态：控制/视频处理器经 Arc<Mutex<>> 共享。
#[derive(Default)]
pub struct HostState {
    pub quality: QualityState,
    /// 服务开关：关闭时拒绝全部入站（GUI 命令 rd_host_start/stop 翻转）。
    pub enabled: bool,
    /// 审批闸开关（运行期可翻转；初始值来自 HostConfig.require_approval）。
    pub require_approval: bool,
    /// 已审批通过的 peer（require_approval 启用时生效）。
    pub approved: HashSet<PeerId>,
    /// 待审批 peer（hello 被拒后登记，approve/deny 消费）。
    pub pending: HashSet<PeerId>,
}

impl HostState {
    /// 校验并采纳质量档位：越界回旧档并返回 Err（调用方回显拒绝）。
    pub fn apply_quality(&mut self, fps: u8, scale: u8, codec: u8) -> Result<QualityState, String> {
        if !(1..=60).contains(&fps) {
            return Err(format!("fps out of range: {fps}"));
        }
        if !(1..=200).contains(&scale) {
            return Err(format!("scale out of range: {scale}"));
        }
        if codec != rd_wire::video::CODEC_RAW_RGBA && codec != rd_wire::video::CODEC_ZLIB_RGBA {
            return Err(format!("unsupported codec: {codec}"));
        }
        self.quality = QualityState { fps, scale, codec };
        Ok(self.quality)
    }
}

pub type SharedState = Arc<Mutex<HostState>>;

/// 会话准入查询：服务关闭一律拒；审批关闭时（approved 集忽略）通过；开启时须在 approved 集。
pub fn admission_allowed(state: &HostState, peer: &PeerId) -> bool {
    state.enabled && (!state.require_approval || state.approved.contains(peer))
}

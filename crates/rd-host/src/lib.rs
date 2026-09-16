//! rd-host：远程桌面 host 侧装配（remote-desktop-plan §2.3）。
//!
//! [RdHost] 注册 /rd/control/1 与 /rd/video/1 处理器：控制通道完成 hello 握手并
//! 处理输入事件（M3：鼠标/键盘/重置经 [rd_input] 注入接缝落到注入器），
//! 视频通道按对端 PeerId 绑定会话后跑帧泵（采集 → 编码 → chunked 发送）。
//! 同 Peer 同时只允许一个活跃会话。
//!
//! 会话准入（M3 简化）：默认接受；authz/服务开关/审批闸在 M6 接入
//! （依赖 wsm 服务总控波合入）。

mod control;
mod file;
mod input;
mod session;
mod sessions;
mod state;

use std::sync::{Arc, Mutex};

use std::path::PathBuf;

use p2p::Node;
use p2p::PeerId;
use p2p_protocol::{ProtocolError, ProtocolId};
use rd_capture::CaptureError;
use rd_capture::CaptureSource;
use rd_clipboard::ClipboardFactory;
use rd_fs::FsService;
use rd_input::InjectorFactory;
use rd_wire::{CONTROL_PROTOCOL_ID, FILE_PROTOCOL_ID, VIDEO_PROTOCOL_ID};
pub use state::QualityState;
use state::{HostState, SharedState};

/// host 装配失败。
#[derive(Debug, thiserror::Error)]
pub enum HostError {
    #[error("protocol id: {0}")]
    Protocol(#[from] ProtocolError),
}

/// host 会话配置。
#[derive(Debug, Clone)]
pub struct HostConfig {
    /// 视频目标帧率（1..=60）。
    pub fps: u8,
    /// 控制通道空闲超时（心跳护栏，秒）；0 表示不启用。
    pub idle_timeout_secs: u64,
    /// 编码：raw=0 原样，zlib=1 deflate 压缩。
    pub codec: u8,
    /// 文件隔离根（viewer 可见的目录树根）。
    pub fs_root: PathBuf,
    /// 会话审批闸：开启后新 viewer hello 被拒（awaiting_approval），
    /// 需 approve(peer) 后重连（RustDesk 临时口令/审批同款模式）。
    pub require_approval: bool,
}

impl Default for HostConfig {
    fn default() -> Self {
        Self {
            fps: 15,
            idle_timeout_secs: 15,
            codec: rd_wire::video::CODEC_RAW_RGBA,
            fs_root: default_fs_root(),
            require_approval: false,
        }
    }
}

/// 默认文件根：`$HOME/Downloads/RD`（商用隔离语义；HOME 缺失回退当前目录 .rd-files）。
fn default_fs_root() -> PathBuf {
    let home = std::env::var_os("HOME").map(PathBuf::from);
    match home {
        Some(h) => h.join("Downloads").join("RD"),
        None => PathBuf::from(".rd-files"),
    }
}

/// host 侧服务装配：持有会话注册表，控制/视频处理器共享。
pub struct RdHost {
    sessions: Arc<Mutex<sessions::HostSessions>>,
    state: SharedState,
    _node: Arc<Node>,
}

impl RdHost {
    /// 默认配置装配（注入器 = macOS 真实注入；非 macOS 为 recording；剪贴板 = 系统）。
    pub fn new(node: Arc<Node>, source: Arc<dyn SourceFactory>) -> Result<Self, HostError> {
        Self::with_config(
            node,
            source,
            default_injector_factory(),
            Arc::new(rd_clipboard::SystemClipboardFactory),
            HostConfig::default(),
        )
    }

    /// 显式配置装配：解析协议 ID 后注册控制（含输入/剪贴板）/视频处理器。
    pub fn with_config(
        node: Arc<Node>,
        source: Arc<dyn SourceFactory>,
        injector: Arc<dyn InjectorFactory>,
        clipboard: Arc<dyn ClipboardFactory>,
        config: HostConfig,
    ) -> Result<Self, HostError> {
        let control_id = ProtocolId::new(CONTROL_PROTOCOL_ID)?;
        let video_id = ProtocolId::new(VIDEO_PROTOCOL_ID)?;
        let file_id = ProtocolId::new(FILE_PROTOCOL_ID)?;
        let sessions = Arc::new(Mutex::new(sessions::HostSessions::default()));
        let state: SharedState = Arc::new(Mutex::new(HostState {
            require_approval: config.require_approval,
            ..HostState::default()
        }));
        let config = Arc::new(config);
        let host = Self {
            sessions: sessions.clone(),
            state: state.clone(),
            _node: node.clone(),
        };
        node.handle_protocol(Arc::new(control::ControlHandler {
            sessions: sessions.clone(),
            state: state.clone(),
            proto: control_id,
            injector,
            clipboard,
            config: config.clone(),
        }));
        node.handle_protocol(Arc::new(session::VideoHandler {
            sessions: sessions.clone(),
            state: state.clone(),
            proto: video_id,
            source,
            config: config.clone(),
        }));
        node.handle_protocol(Arc::new(file::FileHandler {
            sessions,
            proto: file_id,
            fs: FsService::new(config.fs_root.clone()),
            config,
        }));
        Ok(host)
    }

    /// 活跃会话数（观测/测试）。
    pub fn session_count(&self) -> usize {
        self.sessions.lock().map(|g| g.len()).unwrap_or(0)
    }

    /// 审批通过 peer（require_approval 开启后生效）；返回是否确有 pending 消费。
    pub fn approve(&self, peer: &PeerId) -> bool {
        let mut st = self.state.lock().unwrap_or_else(|e| e.into_inner());
        let consumed = st.pending.remove(peer);
        st.approved.insert(*peer);
        consumed
    }

    /// 拒绝审批（仅清 pending，不加入 approved）。
    pub fn deny(&self, peer: &PeerId) -> bool {
        self.state
            .lock()
            .map(|mut st| st.pending.remove(peer))
            .unwrap_or(false)
    }

    /// 待审批 peer 列表（GUI 审批队列数据源）。
    pub fn pending_approvals(&self) -> Vec<PeerId> {
        self.state
            .lock()
            .map(|st| st.pending.iter().copied().collect())
            .unwrap_or_default()
    }

    /// 当前采纳帧率（质量协商读回；测试/E2E 断言用）。
    pub fn active_fps(&self) -> u8 {
        self.state.lock().map(|st| st.quality.fps).unwrap_or(0)
    }

    /// 服务开关翻转（GUI rd_host_start/stop 命令面）。
    pub fn set_enabled(&self, enabled: bool) {
        if let Ok(mut st) = self.state.lock() {
            st.enabled = enabled;
        }
    }

    /// 服务开关读回。
    pub fn state_enabled(&self) -> bool {
        self.state.lock().map(|st| st.enabled).unwrap_or(false)
    }

    /// 审批闸读回。
    pub fn state_require_approval(&self) -> bool {
        self.state
            .lock()
            .map(|st| st.require_approval)
            .unwrap_or(false)
    }

    /// 质量档位直接设置（GUI 命令面；校验失败回旧档）。
    pub fn set_quality(&self, fps: u8, scale: u8, codec: u8) -> Result<QualityState, String> {
        let mut st = self.state.lock().unwrap_or_else(|e| e.into_inner());
        st.apply_quality(fps, scale, codec)
    }

    /// 审批闸运行期翻转。
    pub fn set_require_approval(&self, on: bool) {
        if let Ok(mut st) = self.state.lock() {
            st.require_approval = on;
        }
    }

    /// 服务关闭时停止全部活跃会话（信号 stop 旗标，由处理器各自收尾）。
    pub fn stop_all(&self) {
        let peers: Vec<PeerId> = self.sessions.lock().map(|g| g.peers()).unwrap_or_default();
        if let Ok(g) = self.sessions.lock() {
            for p in peers {
                g.signal_stop(&p);
            }
        }
    }
}

/// 默认注入器工厂：macOS 用 CGEvent 真实注入，其余平台用 recording（本仓库面向 macOS）。
#[cfg(target_os = "macos")]
fn default_injector_factory() -> Arc<dyn InjectorFactory> {
    Arc::new(rd_input::macos::MacInjectorFactory)
}

#[cfg(not(target_os = "macos"))]
fn default_injector_factory() -> Arc<dyn InjectorFactory> {
    Arc::new(rd_input::recording::RecordingInjectorFactory::new())
}

/// 采集源工厂：每会话一个采集实例（真实源按显示器绑定）。
pub trait SourceFactory: Send + Sync {
    fn new_source(&self) -> Result<Box<dyn CaptureSource + Send>, CaptureError>;
}

/// 合成源工厂（测试/E2E）。
pub struct SyntheticFactory {
    pub w: u16,
    pub h: u16,
}

impl SourceFactory for SyntheticFactory {
    fn new_source(&self) -> Result<Box<dyn CaptureSource + Send>, CaptureError> {
        Ok(Box::new(rd_capture::synthetic::SyntheticSource::new(
            self.w, self.h,
        )))
    }
}

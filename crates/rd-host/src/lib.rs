//! rd-host：远程桌面 host 侧装配（remote-desktop-plan §2.3）。
//!
//! [RdHost] 注册 /rd/control/1 与 /rd/video/1 处理器：控制通道完成 hello 握手并
//! 处理输入事件（M3：鼠标/键盘/重置经 [rd_input] 注入接缝落到注入器），
//! 视频通道按对端 PeerId 绑定会话后跑帧泵（采集 → 编码 → chunked 发送）。
//! 同 Peer 同时只允许一个活跃会话。
//!
//! 会话准入（M3 简化）：默认接受；authz/服务开关/审批闸在 M6 接入
//! （依赖 wsm 服务总控波合入）。

mod session;

use std::sync::{Arc, Mutex};

use p2p::Node;
use p2p_protocol::{ProtocolError, ProtocolId};
use rd_capture::CaptureError;
use rd_capture::CaptureSource;
use rd_input::InjectorFactory;
use rd_wire::{CONTROL_PROTOCOL_ID, VIDEO_PROTOCOL_ID};

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
}

impl Default for HostConfig {
    fn default() -> Self {
        Self {
            fps: 15,
            idle_timeout_secs: 15,
            codec: rd_wire::video::CODEC_RAW_RGBA,
        }
    }
}

/// host 侧服务装配：持有会话注册表，控制/视频处理器共享。
pub struct RdHost {
    sessions: Arc<Mutex<session::HostSessions>>,
    _node: Arc<Node>,
}

impl RdHost {
    /// 默认配置装配（注入器 = macOS 真实注入；非 macOS 为 recording）。
    pub fn new(node: Arc<Node>, source: Arc<dyn SourceFactory>) -> Result<Self, HostError> {
        Self::with_config(
            node,
            source,
            default_injector_factory(),
            HostConfig::default(),
        )
    }

    /// 显式配置装配：解析协议 ID 后注册控制（含输入）/视频处理器。
    pub fn with_config(
        node: Arc<Node>,
        source: Arc<dyn SourceFactory>,
        injector: Arc<dyn InjectorFactory>,
        config: HostConfig,
    ) -> Result<Self, HostError> {
        let control_id = ProtocolId::new(CONTROL_PROTOCOL_ID)?;
        let video_id = ProtocolId::new(VIDEO_PROTOCOL_ID)?;
        let sessions = Arc::new(Mutex::new(session::HostSessions::default()));
        let config = Arc::new(config);
        let host = Self {
            sessions: sessions.clone(),
            _node: node.clone(),
        };
        node.handle_protocol(Arc::new(session::ControlHandler {
            sessions: sessions.clone(),
            proto: control_id,
            injector,
            config: config.clone(),
        }));
        node.handle_protocol(Arc::new(session::VideoHandler {
            sessions,
            proto: video_id,
            source,
            config,
        }));
        Ok(host)
    }

    /// 活跃会话数（观测/测试）。
    pub fn session_count(&self) -> usize {
        self.sessions.lock().map(|g| g.len()).unwrap_or(0)
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

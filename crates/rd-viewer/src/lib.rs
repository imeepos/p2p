//! rd-viewer：远程桌面 viewer 侧装配（remote-desktop-plan §2.3）。
//!
//! [RdViewer] 拨号 host：/rd/control/1 完成 hello 握手后开 /rd/video/1，
//! 视频泵解码帧（raw/zlib）交给 [RenderSink]；会话可显式 close。
//! 输入注入（M3）与剪贴板（M4）沿控制通道增量接入。

mod file;
mod session;

use std::sync::Arc;

use crate::file::FileChannel;
use p2p::Node;
use p2p_protocol::ProtocolError;

/// viewer 侧错误。
#[derive(Debug, thiserror::Error)]
pub enum ViewerError {
    #[error("protocol id: {0}")]
    Protocol(#[from] ProtocolError),
    #[error("node: {0}")]
    Node(#[from] p2p::NodeError),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("handshake rejected: {0}")]
    Rejected(String),
    #[error("wire: {0}")]
    Wire(#[from] rd_wire::WireError),
    #[error("decode: {0}")]
    Decode(String),
    #[error("aborted: {0}")]
    Aborted(String),
    #[error("closed")]
    Closed,
}

/// 解码后的一帧画面（RGBA8），交给渲染侧。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodedFrame {
    pub w: u16,
    pub h: u16,
    pub keyframe: bool,
    pub seq: u32,
    pub rgba: Vec<u8>,
}

/// 渲染接缝：viewer 把解码帧交给 GUI/canvas（或测试收集器）。
pub trait RenderSink: Send + Sync {
    fn on_frame(&self, frame: DecodedFrame);
}

/// viewer 服务：每次 connect 产出一个活跃会话。
pub struct RdViewer {
    node: Arc<Node>,
}

impl RdViewer {
    pub fn new(node: Arc<Node>) -> Self {
        Self { node }
    }

    /// 连接 host：握手 → 开视频流 → 起视频泵；返回可 close 的会话。
    pub async fn connect(
        &self,
        peer: p2p::PeerId,
        session_id: String,
        sink: Arc<dyn RenderSink>,
    ) -> Result<ViewerSession, ViewerError> {
        let inner = session::connect(self.node.clone(), peer, session_id, sink).await?;
        Ok(ViewerSession {
            inner,
            fs: FileChannel::new(self.node.clone(), peer),
        })
    }

    /// 连接 host（M4 剪贴板版）：clip 为 viewer 本机剪贴板后端，host 下行写入此处。
    pub async fn connect_full(
        &self,
        peer: p2p::PeerId,
        session_id: String,
        sink: Arc<dyn RenderSink>,
        clip: Option<Arc<tokio::sync::Mutex<dyn rd_clipboard::ClipboardBackend>>>,
    ) -> Result<ViewerSession, ViewerError> {
        let inner = session::connect_full(self.node.clone(), peer, session_id, sink, clip).await?;
        Ok(ViewerSession {
            inner,
            fs: FileChannel::new(self.node.clone(), peer),
        })
    }
}

/// 活跃 viewer 会话：close() 显式关闭（视频泵任务随之退出）。
pub struct ViewerSession {
    inner: session::ViewerSession,
    fs: FileChannel,
}

impl ViewerSession {
    /// 显式关闭：发 Close 控制帧并停止视频泵。
    pub async fn close(self) -> Result<(), ViewerError> {
        self.inner.close().await
    }

    /// 控制写半（M3 输入注入复用）。
    pub fn control(&self) -> Arc<dyn session::ControlWrite> {
        self.inner.control()
    }

    /// 文件通道（M5）：目录浏览/上下传单飞操作。
    pub fn fs(&self) -> &FileChannel {
        &self.fs
    }
}

//! host 会话状态机：控制处理器（握手+控制循环）、视频处理器（帧泵）、
//! 会话注册表（按 PeerId，同一 Peer 单活跃会话）。
//!
//! 采集源只活在视频处理器内：控制处理器只登记会话与停止旗标，
//! 视频流到达后按 PeerId 绑定会话并实例化采集源跑帧泵。

use std::io::Write;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::sessions::HostSessions;
use flate2::write::ZlibEncoder;
use flate2::Compression;
use p2p::PeerId;
use p2p_mux::BoxedStream;
use p2p_protocol::{ProtocolHandler, ProtocolId};
use rd_capture::{CaptureError, CaptureSource};
use rd_wire::io::send_large;
use rd_wire::video::{encode_frame, FrameHeader, Rect};
use tokio::io;

use crate::{HostConfig, SourceFactory};

/// /rd/video/1 处理器：按 PeerId 绑定会话 → 实例化采集源 → 帧泵。
pub struct VideoHandler {
    pub sessions: Arc<Mutex<HostSessions>>,
    pub proto: ProtocolId,
    pub source: Arc<dyn SourceFactory>,
    pub config: Arc<HostConfig>,
}

#[async_trait::async_trait]
impl ProtocolHandler for VideoHandler {
    fn protocol(&self) -> ProtocolId {
        self.proto.clone()
    }

    async fn handle(&self, _stream: BoxedStream) -> io::Result<()> {
        tracing::warn!("rd-host: bare stream without peer identity rejected");
        Ok(())
    }

    async fn handle_inbound(&self, peer: PeerId, mut stream: BoxedStream) -> io::Result<()> {
        let ctx = match self.sessions.lock() {
            Ok(g) => g.get(&peer).map(|c| (c.session_id.clone(), c.stop.clone())),
            Err(_) => None,
        };
        let Some((session_id, stop)) = ctx else {
            tracing::warn!("rd-host: video stream without session from {peer}");
            return Ok(());
        };
        tracing::info!("rd-host: video channel bound to session {session_id}");
        let mut capture = match self.source.new_source() {
            Ok(c) => c,
            Err(e) => {
                tracing::error!("rd-host: capture unavailable for {peer}: {e}");
                if let Ok(g) = self.sessions.lock() {
                    g.signal_stop(&peer)
                };
                return Ok(());
            }
        };
        let result = pump_frames(&mut stream, &mut capture, stop, self.config.as_ref()).await;
        let _ = self.sessions.lock().map(|mut g| g.remove(&peer));
        tracing::info!("rd-host: video channel ended for {peer}");
        result
    }
}

/// 帧泵：采集 → 编码（raw/zlib）→ chunked 发送；stop 置位或写失败即退出。
async fn pump_frames(
    stream: &mut BoxedStream,
    capture: &mut Box<dyn CaptureSource + Send>,
    stop: Arc<AtomicBool>,
    config: &HostConfig,
) -> io::Result<()> {
    let fps = config.fps.clamp(1, 60);
    let period = Duration::from_millis(1000 / fps as u64);
    let keyframe_every = 30u32;
    let mut seq: u32 = 0;
    loop {
        if stop.load(Ordering::Relaxed) {
            break;
        }
        let frame = match capture.next_frame() {
            Ok(f) => f,
            Err(e) => {
                tracing::error!("rd-host: capture error: {e}");
                break;
            }
        };
        if let Err(e) = frame.validate() {
            tracing::error!("rd-host: invalid frame: {e}");
            break;
        }
        seq = seq.wrapping_add(1);
        let keyframe = seq % keyframe_every == 1;
        let payload = match config.codec {
            rd_wire::video::CODEC_ZLIB_RGBA => match zlib_compress(&frame.rgba) {
                Ok(p) => p,
                Err(e) => {
                    tracing::error!("rd-host: zlib encode failed: {e}");
                    break;
                }
            },
            _ => frame.rgba,
        };
        let header = FrameHeader {
            seq,
            ts_ms: now_ms(),
            w: frame.w,
            h: frame.h,
            keyframe,
            codec: config.codec,
            rects: vec![Rect {
                x: 0,
                y: 0,
                w: frame.w,
                h: frame.h,
            }],
        };
        let bytes = match encode_frame(&header, &payload) {
            Ok(b) => b,
            Err(e) => {
                tracing::error!("rd-host: frame encode failed: {e}");
                break;
            }
        };
        if let Err(e) = send_large(stream, &bytes).await {
            tracing::warn!("rd-host: video send failed: {e}");
            break;
        }
        tokio::time::sleep(period).await;
    }
    Ok(())
}

/// deflate 压缩（zlib 容器，level 3：速度与压缩比折中）。
fn zlib_compress(rgba: &[u8]) -> Result<Vec<u8>, CaptureError> {
    let mut enc = ZlibEncoder::new(Vec::new(), Compression::new(3));
    enc.write_all(rgba)
        .map_err(|e| CaptureError::Io(format!("zlib write: {e}")))?;
    enc.finish()
        .map_err(|e| CaptureError::Io(format!("zlib finish: {e}")))
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

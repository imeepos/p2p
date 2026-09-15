//! host 会话状态机：控制处理器（握手+控制循环）、视频处理器（帧泵）、
//! 会话注册表（按 PeerId，同一 Peer 单活跃会话）。
//!
//! 采集源只活在视频处理器内：控制处理器只登记会话与停止旗标，
//! 视频流到达后按 PeerId 绑定会话并实例化采集源跑帧泵。

use std::io::Write;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use flate2::write::ZlibEncoder;
use flate2::Compression;
use p2p::PeerId;
use p2p_mux::BoxedStream;
use p2p_protocol::{ProtocolHandler, ProtocolId};
use rd_capture::{CaptureError, CaptureSource};
use crate::input::InputDispatch;
use crate::sessions::HostSessions;
use rd_input::InjectorFactory;
use rd_wire::io::{recv_control, send_control, send_large};
use rd_wire::video::{encode_frame, FrameHeader, Rect};
use rd_wire::{ControlMsg, Role};
use tokio::io;

use crate::{HostConfig, SourceFactory};

/// /rd/control/1 处理器：hello 握手 → 控制循环（输入注入/Close/空闲超时退出）。
pub struct ControlHandler {
    pub sessions: Arc<Mutex<HostSessions>>,
    pub proto: ProtocolId,
    pub injector: Arc<dyn InjectorFactory>,
    pub config: Arc<HostConfig>,
}

#[async_trait::async_trait]
impl ProtocolHandler for ControlHandler {
    fn protocol(&self) -> ProtocolId {
        self.proto.clone()
    }

    async fn handle(&self, _stream: BoxedStream) -> io::Result<()> {
        tracing::warn!("rd-host: bare stream without peer identity rejected");
        Ok(())
    }

    async fn handle_inbound(&self, peer: PeerId, mut stream: BoxedStream) -> io::Result<()> {
        let hello = match recv_control(&mut stream).await {
            Ok(ControlMsg::Hello {
                role: Role::Viewer,
                session_id,
                ..
            }) => session_id,
            Ok(other) => {
                tracing::warn!("rd-host: unexpected first control msg from {peer}: {other:?}");
                return Ok(());
            }
            Err(e) => return Err(e),
        };
        let mut dispatch = match self.injector.new_injector() {
            Ok(i) => InputDispatch::new(i),
            Err(e) => {
                tracing::error!("rd-host: input injector unavailable for {peer}: {e}");
                let _ = send_control(
                    &mut stream,
                    &ControlMsg::HelloAck {
                        ok: false,
                        reason: Some(format!("input unavailable: {e}")),
                        session_id: hello,
                    },
                )
                .await;
                return Ok(());
            }
        };
        let registered = match self.sessions.lock() {
            Ok(mut g) => g.insert(peer, hello.clone()),
            Err(poison) => poison.into_inner().insert(peer, hello.clone()),
        };
        if let Err(reason) = registered {
            tracing::warn!("rd-host: reject {peer}: {reason}");
            let _ = send_control(
                &mut stream,
                &ControlMsg::HelloAck {
                    ok: false,
                    reason: Some(reason),
                    session_id: hello,
                },
            )
            .await;
            return Ok(());
        }
        let ack = ControlMsg::HelloAck {
            ok: true,
            reason: None,
            session_id: hello.clone(),
        };
        if let Err(e) = send_control(&mut stream, &ack).await {
            let _ = self.sessions.lock().map(|mut g| g.remove(&peer));
            return Err(e);
        }
        tracing::info!("rd-host: session {hello} started with {peer}");
        let idle = Duration::from_secs(self.config.idle_timeout_secs);
        loop {
            let next = if self.config.idle_timeout_secs > 0 {
                tokio::time::timeout(idle, recv_control(&mut stream)).await
            } else {
                Ok(recv_control(&mut stream).await)
            };
            match next {
                Ok(Ok(ControlMsg::Close { .. })) => break,
                Ok(Ok(ControlMsg::InputMouse {
                    x,
                    y,
                    buttons: mask,
                    wheel_dx,
                    wheel_dy,
                })) => {
                    dispatch.mouse(x, y, mask, wheel_dx, wheel_dy);
                }
                Ok(Ok(ControlMsg::InputKey {
                    code,
                    down,
                    modifiers,
                })) => {
                    dispatch.key(code, down, modifiers);
                }
                Ok(Ok(ControlMsg::InputKeyReset)) => {
                    dispatch.reset();
                }
                Ok(Ok(_)) => {}
                Ok(Err(e)) => {
                    tracing::warn!("rd-host: control read error: {e}");
                    break;
                }
                Err(_) => {
                    tracing::warn!("rd-host: control idle timeout, closing session {hello}");
                    break;
                }
            }
        }
        dispatch.reset();
        let _ = self.sessions.lock().map(|mut g| g.remove(&peer));
        tracing::info!("rd-host: session {hello} ended with {peer}");
        Ok(())
    }
}

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

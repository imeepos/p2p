//! host 会话状态机：控制处理器（握手+控制循环）、视频处理器（帧泵）、
//! 会话注册表（按 PeerId，同一 Peer 单活跃会话）。
//!
//! 采集源只活在视频处理器内：控制处理器只登记会话与停止旗标，
//! 视频流到达后按 PeerId 绑定会话并实例化采集源跑帧泵。

use std::collections::HashMap;
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
use rd_input::{InjectError, InjectorFactory, InputInjector, MouseButton};
use rd_wire::io::{recv_control, send_control, send_large};
use rd_wire::video::{encode_frame, FrameHeader, Rect};
use rd_wire::{ControlMsg, Role};
use tokio::io;

use crate::{HostConfig, SourceFactory};

/// 会话注册表：peer → 会话上下文。
#[derive(Default)]
pub struct HostSessions {
    inner: HashMap<PeerId, HostCtx>,
}

/// 单会话上下文：会话 id + 停止旗标（控制/视频任一侧退出即置位）。
pub struct HostCtx {
    pub session_id: String,
    pub stop: Arc<AtomicBool>,
}

impl HostSessions {
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /// 登记会话；同 peer 已有活跃会话则拒绝。
    pub fn insert(&mut self, peer: PeerId, session_id: String) -> Result<(), String> {
        if self.inner.contains_key(&peer) {
            return Err("session already active for peer".into());
        }
        let stop = Arc::new(AtomicBool::new(false));
        self.inner.insert(peer, HostCtx { session_id, stop });
        Ok(())
    }

    pub fn get(&self, peer: &PeerId) -> Option<&HostCtx> {
        self.inner.get(peer)
    }

    /// 置位停止旗标（通知对侧退出）。
    pub fn signal_stop(&self, peer: &PeerId) {
        if let Some(ctx) = self.inner.get(peer) {
            ctx.stop.store(true, Ordering::Relaxed);
        }
    }

    /// 移除会话（幂等）。
    pub fn remove(&mut self, peer: &PeerId) -> Option<HostCtx> {
        self.inner.remove(peer)
    }
}

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
        let mut injector = match self.injector.new_injector() {
            Ok(i) => i,
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
        let mut buttons = [false; 3];
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
                    // 顺序：先移动到位 → 再按键/滚轮（点击落在目标位置）
                    if let Err(e) = injector.mouse_move(x, y) {
                        warn_inject("mouse_move", &e);
                    }
                    let cur = MouseButton::from_mask(mask);
                    for (i, down) in cur.into_iter().enumerate() {
                        if down != buttons[i] {
                            inject_mouse_button(&mut *injector, btn_at(i), down);
                            buttons[i] = down;
                        }
                    }
                    if wheel_dx != 0 || wheel_dy != 0 {
                        if let Err(e) = injector.mouse_wheel(wheel_dx, wheel_dy) {
                            warn_inject("mouse_wheel", &e);
                        }
                    }
                }
                Ok(Ok(ControlMsg::InputKey {
                    code,
                    down,
                    modifiers,
                })) => {
                    if let Err(e) = injector.key(code, down, modifiers) {
                        warn_inject("key", &e);
                    }
                }
                Ok(Ok(ControlMsg::InputKeyReset)) => {
                    if let Err(e) = injector.reset_keys() {
                        warn_inject("reset_keys", &e);
                    }
                    buttons = [false; 3];
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
        let _ = injector.reset_keys();
        let _ = self.sessions.lock().map(|mut g| g.remove(&peer));
        tracing::info!("rd-host: session {hello} ended with {peer}");
        Ok(())
    }
}

fn inject_mouse_button(inj: &mut (impl InputInjector + ?Sized), btn: MouseButton, down: bool) {
    if let Err(e) = inj.mouse_button(btn, down) {
        warn_inject("mouse_button", &e);
    }
}

fn btn_at(i: usize) -> MouseButton {
    match i {
        0 => MouseButton::Left,
        1 => MouseButton::Right,
        _ => MouseButton::Middle,
    }
}

fn warn_inject(op: &str, e: &InjectError) {
    tracing::warn!("rd-host: {op} inject failed: {e}");
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

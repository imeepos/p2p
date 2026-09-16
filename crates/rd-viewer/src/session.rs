//! viewer 会话：拨号/握手/视频泵/关闭。
//! 控制写半经 [ControlWrite] 抽象（内部 tokio Mutex 串行化），M3 输入注入直接复用。

use std::io::Read;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use flate2::read::ZlibDecoder;
use p2p::Node;
use p2p_mux::BoxedStream;
use rd_clipboard::ClipboardBackend;
use rd_wire::io::{recv_control, recv_large, send_control};
use rd_wire::video::{decode_frame, CODEC_RAW_RGBA, CODEC_ZLIB_RGBA};
use rd_wire::{ControlMsg, Role};
use tokio::sync::Mutex;

use crate::{DecodedFrame, RenderSink, ViewerError};

/// 控制写半：close 后写入仍返回 IoError（对端已断，调用方处理）。
#[async_trait::async_trait]
pub trait ControlWrite: Send + Sync {
    async fn send(&self, msg: ControlMsg) -> Result<(), ViewerError>;

    /// 输入便捷面（M3）：鼠标绝对坐标事件。
    async fn mouse(
        &self,
        x: u16,
        y: u16,
        buttons: u8,
        wheel_dx: i8,
        wheel_dy: i8,
    ) -> Result<(), ViewerError> {
        self.send(ControlMsg::InputMouse {
            x,
            y,
            buttons,
            wheel_dx,
            wheel_dy,
        })
        .await
    }

    /// 输入便捷面（M3）：键盘事件（USB HID 键码）。
    async fn key(&self, code: u16, down: bool, modifiers: u8) -> Result<(), ViewerError> {
        self.send(ControlMsg::InputKey {
            code,
            down,
            modifiers,
        })
        .await
    }

    /// 输入便捷面（M3）：释放全部按键（失焦/断线时调用）。
    async fn key_reset(&self) -> Result<(), ViewerError> {
        self.send(ControlMsg::InputKeyReset).await
    }

    /// 剪贴板便捷面（M4）：推送本机文本剪贴板。
    async fn clipboard(&self, text: String) -> Result<(), ViewerError> {
        self.send(ControlMsg::Clipboard { text }).await
    }

    /// 质量协商便捷面（M6）：请求档位（fps/缩放%/codec）。
    async fn quality(&self, fps: u8, scale: u8, codec: u8) -> Result<(), ViewerError> {
        self.send(ControlMsg::Quality { fps, scale, codec }).await
    }
}

/// 活跃会话：控制写半 + 控制读任务 + 视频泵任务 + 停止旗标。
pub struct ViewerSession {
    ctl: Arc<dyn ControlWrite>,
    ctl_task: tokio::task::JoinHandle<()>,
    video_task: tokio::task::JoinHandle<()>,
    stop: Arc<AtomicBool>,
}

impl ViewerSession {
    pub fn control(&self) -> Arc<dyn ControlWrite> {
        self.ctl.clone()
    }

    /// 显式关闭：发 Close → 置停止旗标 → 等视频泵退出。
    pub async fn close(self) -> Result<(), ViewerError> {
        let _ = self
            .ctl
            .send(ControlMsg::Close {
                reason: "viewer closing".into(),
            })
            .await;
        self.stop.store(true, Ordering::Relaxed);
        self.ctl_task.abort();
        let _ = self.video_task.await;
        Ok(())
    }
}

/// 控制写半实现：独占写半（tokio Mutex 串行化并发 send）。
pub struct CtlHalf {
    stream: Mutex<tokio::io::WriteHalf<BoxedStream>>,
}

impl CtlHalf {
    fn new(stream: tokio::io::WriteHalf<BoxedStream>) -> Self {
        Self {
            stream: Mutex::new(stream),
        }
    }
}

#[async_trait::async_trait]
impl ControlWrite for CtlHalf {
    async fn send(&self, msg: ControlMsg) -> Result<(), ViewerError> {
        let mut s = self.stream.lock().await;
        send_control(&mut *s, &msg).await.map_err(ViewerError::Io)
    }
}

/// 控制读任务：处理 host 下行消息（剪贴板写入本机后端；Close 退出）。
async fn control_read_loop(
    mut read_half: tokio::io::ReadHalf<BoxedStream>,
    clip: Option<Arc<Mutex<dyn ClipboardBackend>>>,
    stop: Arc<AtomicBool>,
) {
    loop {
        if stop.load(Ordering::Relaxed) {
            break;
        }
        let msg = match recv_control(&mut read_half).await {
            Ok(m) => m,
            Err(e) => {
                if !stop.load(Ordering::Relaxed) {
                    tracing::warn!("rd-viewer: control recv ended: {e}");
                }
                break;
            }
        };
        match msg {
            ControlMsg::Clipboard { text } => {
                if let Some(clip) = &clip {
                    let mut g = clip.lock().await;
                    if let Err(e) = g.write_text(&text) {
                        tracing::warn!("rd-viewer: clipboard write failed: {e}");
                    }
                }
            }
            ControlMsg::Close { .. } => break,
            _ => {}
        }
    }
}

/// 拨号流程（不带剪贴板 sink）：等价 connect_full(None)。
pub async fn connect(
    node: Arc<Node>,
    peer: p2p::PeerId,
    session_id: String,
    sink: Arc<dyn RenderSink>,
) -> Result<ViewerSession, ViewerError> {
    connect_full(node, peer, session_id, sink, None).await
}

/// 拨号流程（M4）：control 流 hello 握手 → split 读写半 → video 流 → 泵。
/// clip 为 viewer 本机剪贴板后端（可选）：host 下行剪贴板写到这里。
pub async fn connect_full(
    node: Arc<Node>,
    peer: p2p::PeerId,
    session_id: String,
    sink: Arc<dyn RenderSink>,
    clip: Option<Arc<Mutex<dyn ClipboardBackend>>>,
) -> Result<ViewerSession, ViewerError> {
    let mut ctl_stream = node
        .new_stream(peer, rd_wire::control_protocol_id()?)
        .await?;
    send_control(
        &mut ctl_stream,
        &ControlMsg::Hello {
            v: rd_wire::PROTOCOL_VERSION,
            role: Role::Viewer,
            session_id: session_id.clone(),
            caps: rd_wire::Caps {
                audio: false,
                file: true,
                clipboard: true,
            },
        },
    )
    .await?;
    let ack = recv_control(&mut ctl_stream).await?;
    match ack {
        ControlMsg::HelloAck {
            ok: true,
            session_id: sid,
            ..
        } if sid == session_id => {}
        ControlMsg::HelloAck {
            ok: false, reason, ..
        } => {
            let why = reason.unwrap_or_else(|| "no reason".into());
            return Err(ViewerError::Rejected(why));
        }
        other => return Err(ViewerError::Decode(format!("unexpected ack: {other:?}"))),
    }
    let (read_half, write_half) = tokio::io::split(ctl_stream);
    let ctl = CtlHalf::new(write_half);
    let video_stream = node.new_stream(peer, rd_wire::video_protocol_id()?).await?;
    let stop = Arc::new(AtomicBool::new(false));
    let ctl_stop = stop.clone();
    let video_stop = stop.clone();
    let ctl_task = tokio::spawn(control_read_loop(read_half, clip, ctl_stop));
    let video_task = tokio::spawn(video_pump(video_stream, sink, video_stop));
    Ok(ViewerSession {
        ctl: Arc::new(ctl),
        ctl_task,
        video_task,
        stop,
    })
}

/// 视频泵：chunked 收帧 → 解码（raw/zlib）→ sink；未同步（未见 keyframe）前丢弃 delta。
async fn video_pump(mut stream: BoxedStream, sink: Arc<dyn RenderSink>, stop: Arc<AtomicBool>) {
    let mut synced = false;
    loop {
        if stop.load(Ordering::Relaxed) {
            break;
        }
        let bytes = match recv_large(&mut stream).await {
            Ok(b) => b,
            Err(e) => {
                if !stop.load(Ordering::Relaxed) {
                    tracing::warn!("rd-viewer: video recv ended: {e}");
                }
                break;
            }
        };
        let (header, payload) = match decode_frame(&bytes) {
            Ok(p) => p,
            Err(e) => {
                tracing::warn!("rd-viewer: frame decode failed: {e}");
                continue;
            }
        };
        if header.keyframe {
            synced = true;
        } else if !synced {
            continue; // 未同步 delta 无意义，按协议约定丢弃
        }
        let rgba = match header.codec {
            CODEC_RAW_RGBA => payload,
            CODEC_ZLIB_RGBA => match zlib_decompress(&payload) {
                Ok(p) => p,
                Err(e) => {
                    tracing::warn!("rd-viewer: zlib inflate failed: {e}");
                    continue;
                }
            },
            other => {
                tracing::warn!("rd-viewer: unsupported codec {other}, drop frame");
                continue;
            }
        };
        sink.on_frame(DecodedFrame {
            w: header.w,
            h: header.h,
            keyframe: header.keyframe,
            seq: header.seq,
            rgba,
        });
    }
}

fn zlib_decompress(bytes: &[u8]) -> Result<Vec<u8>, ViewerError> {
    let mut dec = ZlibDecoder::new(bytes);
    let mut out = Vec::new();
    dec.read_to_end(&mut out)
        .map_err(|e| ViewerError::Decode(format!("inflate: {e}")))?;
    Ok(out)
}

/// 握手探测：hello → hello_ack（含 awaiting_approval 等拒绝原因上抛）即关流。
pub async fn probe(
    node: Arc<Node>,
    peer: p2p::PeerId,
    session_id: String,
) -> Result<String, ViewerError> {
    let mut stream = node
        .new_stream(peer, rd_wire::control_protocol_id()?)
        .await?;
    send_control(
        &mut stream,
        &ControlMsg::Hello {
            v: rd_wire::PROTOCOL_VERSION,
            role: Role::Viewer,
            session_id: session_id.clone(),
            caps: rd_wire::Caps {
                audio: false,
                file: true,
                clipboard: true,
            },
        },
    )
    .await?;
    let ack = recv_control(&mut stream).await?;
    match ack {
        ControlMsg::HelloAck {
            ok: true,
            session_id: sid,
            ..
        } if sid == session_id => Ok(sid),
        ControlMsg::HelloAck {
            ok: false, reason, ..
        } => Err(ViewerError::Rejected(
            reason.unwrap_or_else(|| "no reason".into()),
        )),
        other => Err(ViewerError::Decode(format!("unexpected ack: {other:?}"))),
    }
}

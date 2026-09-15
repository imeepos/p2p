//! viewer 会话：拨号/握手/视频泵/关闭。
//! 控制写半经 [ControlWrite] 抽象（内部 tokio Mutex 串行化），M3 输入注入直接复用。

use std::io::Read;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use flate2::read::ZlibDecoder;
use p2p::Node;
use p2p_mux::BoxedStream;
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
}

/// 活跃会话：控制写半 + 视频泵任务 + 停止旗标。
pub struct ViewerSession {
    ctl: Arc<dyn ControlWrite>,
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
        let _ = self.video_task.await;
        Ok(())
    }
}

/// 控制写半实现：独占控制流（tokio Mutex 串行化并发 send）。
pub struct CtlHalf {
    stream: Mutex<BoxedStream>,
}

impl CtlHalf {
    fn new(stream: BoxedStream) -> Self {
        Self {
            stream: Mutex::new(stream),
        }
    }

    /// 握手期独占访问（hello/hello_ack 之后不再使用）。
    async fn lock(&self) -> tokio::sync::MutexGuard<'_, BoxedStream> {
        self.stream.lock().await
    }
}

#[async_trait::async_trait]
impl ControlWrite for CtlHalf {
    async fn send(&self, msg: ControlMsg) -> Result<(), ViewerError> {
        let mut s = self.stream.lock().await;
        send_control(&mut *s, &msg).await.map_err(ViewerError::Io)
    }
}

/// 拨号流程：control 流 hello 握手 → video 流 → 视频泵。
pub async fn connect(
    node: Arc<Node>,
    peer: p2p::PeerId,
    session_id: String,
    sink: Arc<dyn RenderSink>,
) -> Result<ViewerSession, ViewerError> {
    let ctl_stream = node
        .new_stream(peer, rd_wire::control_protocol_id()?)
        .await?;
    let ctl = CtlHalf::new(ctl_stream);
    {
        let mut s = ctl.lock().await;
        send_control(
            &mut *s,
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
        let ack = recv_control(&mut *s).await?;
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
    }
    let video_stream = node.new_stream(peer, rd_wire::video_protocol_id()?).await?;
    let stop = Arc::new(AtomicBool::new(false));
    let task_stop = stop.clone();
    let video_task = tokio::spawn(video_pump(video_stream, sink, task_stop));
    Ok(ViewerSession {
        ctl: Arc::new(ctl),
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

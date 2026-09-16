//! /rd/control/1 控制处理器：hello 握手 → 控制循环（输入/剪贴板/Close/空闲超时）。
//!
//! 帧读经独立 reader 任务 + mpsc 通道：select! 与 read_frame 竞态会截断帧
//! （read_frame 非 cancel-safe），消息入队不丢。剪贴板双向：viewer 消息写入
//! 本机；本机外部变更经轮询 diff 推给 viewer（回声抑制靠 last_clip 追踪）。

use std::sync::{Arc, Mutex};
use std::time::Duration;

use p2p::PeerId;
use p2p_mux::BoxedStream;
use p2p_protocol::{ProtocolHandler, ProtocolId};
use rd_clipboard::{ClipboardBackend, ClipboardFactory};
use rd_input::InjectorFactory;
use rd_wire::io::{recv_control, send_control};
use rd_wire::{ControlMsg, Role};
use tokio::io;

use crate::input::InputDispatch;
use crate::sessions::HostSessions;
use crate::state::{admission_allowed, SharedState};
use crate::HostConfig;

/// 剪贴板轮询间隔（文本级变更探测，500ms 商用可感粒度）。
const CLIP_POLL_INTERVAL: Duration = Duration::from_millis(500);

/// /rd/control/1 处理器。
pub struct ControlHandler {
    pub sessions: Arc<Mutex<HostSessions>>,
    pub state: SharedState,
    pub proto: ProtocolId,
    pub injector: Arc<dyn InjectorFactory>,
    pub clipboard: Arc<dyn ClipboardFactory>,
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
        // 审批闸：require_approval 且 peer 未批准 → 登记 pending 并拒（viewer 批准后重连）。
        // 锁在纯同步块内释放（guard 非 Send，禁跨 await）。
        let admitted = {
            let mut st = self.state.lock().unwrap_or_else(|e| e.into_inner());
            if admission_allowed(&st, self.config.require_approval, &peer) {
                true
            } else {
                st.pending.insert(peer);
                false
            }
        };
        if !admitted {
            tracing::info!(
                audit_event = "session_awaiting_approval",
                peer = %peer,
                session_id = %hello
            );
            reject(&mut stream, &hello, "awaiting_approval".into()).await;
            return Ok(());
        }
        let mut dispatch = match self.injector.new_injector() {
            Ok(i) => InputDispatch::new(i),
            Err(e) => {
                tracing::error!("rd-host: input injector unavailable for {peer}: {e}");
                reject(&mut stream, &hello, format!("input unavailable: {e}")).await;
                return Ok(());
            }
        };
        let mut clip = match self.clipboard.new_clipboard() {
            Ok(c) => c,
            Err(e) => {
                tracing::error!("rd-host: clipboard unavailable for {peer}: {e}");
                reject(&mut stream, &hello, format!("clipboard unavailable: {e}")).await;
                return Ok(());
            }
        };
        let registered = match self.sessions.lock() {
            Ok(mut g) => g.insert(peer, hello.clone()),
            Err(poison) => poison.into_inner().insert(peer, hello.clone()),
        };
        if let Err(reason) = registered {
            tracing::warn!("rd-host: reject {peer}: {reason}");
            reject(&mut stream, &hello, reason).await;
            return Ok(());
        }
        let ack = ControlMsg::HelloAck {
            ok: true,
            reason: None,
            session_id: hello.clone(),
        };
        if let Err(e) = send_control(&mut stream, &ack).await {
            let _ = self.sessions.lock().map(|mut g| g.remove_if(&peer, &hello));
            return Err(e);
        }
        tracing::info!(
            audit_event = "session_open",
            peer = %peer,
            session_id = %hello
        );
        let outcome = control_loop(
            stream,
            &mut dispatch,
            &mut *clip,
            &peer,
            self.config.as_ref(),
            self.state.clone(),
        )
        .await;
        let _ = self.sessions.lock().map(|mut g| g.remove_if(&peer, &hello));
        tracing::info!(
            audit_event = "session_close",
            peer = %peer,
            session_id = %hello,
            outcome = ?outcome
        );
        Ok(())
    }
}

/// 握手拒绝：hello_ack{ok:false} 后关流。
async fn reject(stream: &mut BoxedStream, hello: &str, reason: String) {
    let _ = send_control(
        stream,
        &ControlMsg::HelloAck {
            ok: false,
            reason: Some(reason),
            session_id: hello.to_string(),
        },
    )
    .await;
}

/// 控制主循环：reader 任务 → mpsc 队列 ↔ 剪贴板轮询（select 双分支皆 cancel-safe）。
async fn control_loop(
    stream: BoxedStream,
    dispatch: &mut InputDispatch,
    clip: &mut (impl ClipboardBackend + ?Sized),
    peer: &PeerId,
    config: &HostConfig,
    state: SharedState,
) -> io::Result<()> {
    let (mut read_half, mut write_half) = tokio::io::split(stream);
    let (tx, mut rx) = tokio::sync::mpsc::channel::<Result<ControlMsg, io::Error>>(64);
    let reader = tokio::spawn(async move {
        loop {
            match recv_control(&mut read_half).await {
                Ok(msg) => {
                    if tx.send(Ok(msg)).await.is_err() {
                        break;
                    }
                }
                Err(e) => {
                    let _ = tx.send(Err(e)).await;
                    break;
                }
            }
        }
    });
    let mut last_clip: Option<String> = None;
    let mut ticker = tokio::time::interval(CLIP_POLL_INTERVAL);
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let idle = Duration::from_secs(config.idle_timeout_secs);
    loop {
        let recv_fut = tokio::time::timeout(idle, rx.recv());
        tokio::select! {
            res = recv_fut => {
                match res {
                    Ok(Some(Ok(msg))) => match msg {
                        ControlMsg::Close { .. } => break,
                        ControlMsg::Clipboard { text } => {
                            if let Err(e) = clip.write_text(&text) {
                                tracing::warn!("rd-host: clipboard write failed: {e}");
                            }
                            last_clip = Some(text);
                        }
                        ControlMsg::InputMouse { x, y, buttons: mask, wheel_dx, wheel_dy } => {
                            dispatch.mouse(x, y, mask, wheel_dx, wheel_dy);
                        }
                        ControlMsg::InputKey { code, down, modifiers } => {
                            dispatch.key(code, down, modifiers);
                        }
                        ControlMsg::InputKeyReset => {
                            dispatch.reset();
                        }
                        ControlMsg::Quality { fps, scale, codec } => {
                            // 锁内只算应答（guard 非 Send 禁跨 await），锁外发送
                            let ack = {
                                let mut st = state.lock().unwrap_or_else(|e| e.into_inner());
                                match st.apply_quality(fps, scale, codec) {
                                    Ok(q) => ControlMsg::QualityAck {
                                        fps: q.fps,
                                        scale: q.scale,
                                        codec: q.codec,
                                    },
                                    Err(e) => {
                                        tracing::warn!("rd-host: quality rejected: {e}");
                                        ControlMsg::QualityAck {
                                            fps: st.quality.fps,
                                            scale: st.quality.scale,
                                            codec: st.quality.codec,
                                        }
                                    }
                                }
                            };
                            let _ = send_control(&mut write_half, &ack).await;
                        }
                        _ => {}
                    },
                    Ok(Some(Err(e))) => {
                        tracing::warn!("rd-host: control read error: {e}");
                        break;
                    }
                    Ok(None) => break,
                    Err(_) => {
                        tracing::warn!("rd-host: control idle timeout (peer {peer})");
                        break;
                    }
                }
            }
            _ = ticker.tick() => {
                if let Err(e) = poll_clipboard(clip, &mut write_half, &mut last_clip).await {
                    tracing::warn!("rd-host: clipboard poll/send failed: {e}");
                }
            }
        }
    }
    drop(rx);
    reader.abort();
    dispatch.reset();
    Ok(())
}

/// 剪贴板轮询：系统侧变更 → 发送给 viewer；回声抑制靠 last_clip 追踪。
async fn poll_clipboard(
    clip: &mut (impl ClipboardBackend + ?Sized),
    write_half: &mut tokio::io::WriteHalf<BoxedStream>,
    last_clip: &mut Option<String>,
) -> io::Result<()> {
    let cur = clip
        .read_text()
        .map_err(|e| io::Error::other(format!("read: {e}")))?;
    if let Some(text) = cur {
        if last_clip.as_deref() != Some(text.as_str()) {
            send_control(write_half, &ControlMsg::Clipboard { text: text.clone() }).await?;
            *last_clip = Some(text);
        }
    }
    Ok(())
}

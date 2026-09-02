//! RelayClient：reserve/connect 状态机 + Punch 信令收发，对 RelayLink 接缝编程。
//!
//! 状态机：reserve 发 Reserve 收 Reserved 发放 CircuitId；connect 发 Connect 收
//! Bound 后该流即电路对端；punch 走控制流，入站信号经事件队列交付调用方。

use std::sync::Arc;
use std::time::Duration;

use p2p_mux::BoxedStream;
use tokio::io::{split, ReadHalf};
use tokio::sync::{mpsc, oneshot, Mutex};

use crate::error::{error_from_wire, RelayError};
use crate::frame::{read_msg, write_msg};
use crate::link::RelayLink;
use crate::messages::{errcode, relay_msg::Kind, PunchAck, RelayMsg};
use crate::state::CtrlWrite;
use crate::CircuitId;

/// 控制请求回包超时。
const REPLY_TIMEOUT: Duration = Duration::from_secs(5);
/// 事件队列容量；满则丢事件并留日志（不反压控制流）。
const EVENT_CAPACITY: usize = 32;

/// 异步事件：入站打洞信令与控制链路关闭。
#[derive(Debug)]
pub enum RelayEvent {
    PunchReq(crate::messages::PunchReq),
    PunchAck(crate::messages::PunchAck),
    ControlClosed,
}

type PendingSlot = Arc<Mutex<Option<oneshot::Sender<RelayMsg>>>>;

struct CtrlChannel {
    write: Arc<CtrlWrite>,
    pending: PendingSlot,
}

/// 中继客户端；一个实例对应一条到 relay 的链路。
pub struct RelayClient {
    link: Box<dyn RelayLink>,
    ctrl: Option<CtrlChannel>,
    events_tx: mpsc::Sender<RelayEvent>,
    events_rx: mpsc::Receiver<RelayEvent>,
}

impl RelayClient {
    pub fn new(link: Box<dyn RelayLink>) -> Self {
        let (events_tx, events_rx) = mpsc::channel(EVENT_CAPACITY);
        Self {
            link,
            ctrl: None,
            events_tx,
            events_rx,
        }
    }

    /// 底层链路声明的对端标识。
    pub fn peer_id(&self) -> &str {
        self.link.peer_id()
    }

    /// 申请电路；allowed_joiner 指定允许接入的对端（空串 = 仅本 Peer 可接入）。
    pub async fn reserve(
        &mut self,
        ttl: Duration,
        allowed_joiner: &str,
    ) -> Result<CircuitId, RelayError> {
        let reply = self
            .control_roundtrip(
                RelayMsg::reserve(ttl.as_secs().max(1), allowed_joiner),
                "reserve",
            )
            .await?;
        match reply.kind {
            Some(Kind::Reserved(r)) => Ok(CircuitId(r.circuit_id)),
            Some(Kind::Reject(r)) => Err(error_from_wire(r.code, r.message)),
            _ => Err(RelayError::Protocol("unexpected reply to reserve".into())),
        }
    }

    /// 接入电路；成功后返回的流与同号对端字节互通（密文搬运）。
    pub async fn connect(&mut self, circuit: CircuitId) -> Result<BoxedStream, RelayError> {
        let mut s = self.link.open_stream().await?;
        write_msg(&mut s, &RelayMsg::connect(circuit.0)).await?;
        let reply = tokio::time::timeout(REPLY_TIMEOUT, read_msg(&mut s))
            .await
            .map_err(|_| RelayError::Timeout("connect"))?;
        let Some(msg) = reply.map_err(RelayError::from)? else {
            return Err(RelayError::LinkClosed);
        };
        match msg.kind {
            Some(Kind::Bound(b)) if b.circuit_id == circuit.0 => {
                tracing::debug!(peer = %self.link.peer_id(), circuit = circuit.0, "circuit bound");
                Ok(s)
            }
            Some(Kind::Bound(_)) => Err(RelayError::Protocol("bound circuit id mismatch".into())),
            Some(Kind::Reject(r)) => Err(error_from_wire(r.code, r.message)),
            _ => Err(RelayError::Protocol("unexpected reply to connect".into())),
        }
    }

    /// 主动发起打洞协调；返回经 relay 改写的对端 PunchAck（含对端观测地址）。
    pub async fn request_punch(
        &mut self,
        target: &str,
        addrs: Vec<String>,
    ) -> Result<PunchAck, RelayError> {
        let reply = self
            .control_roundtrip(RelayMsg::punch_req(target, addrs), "punch-ack")
            .await?;
        match reply.kind {
            Some(Kind::PunchAck(a)) => Ok(a),
            Some(Kind::Reject(r)) => Err(error_from_wire(r.code, r.message)),
            _ => Err(RelayError::Protocol("unexpected reply to punch-req".into())),
        }
    }

    /// 被动方回 PunchAck（转发失败由对端超时发现）。
    pub async fn reply_punch(&mut self, ack: PunchAck) -> Result<(), RelayError> {
        let ch = self.ensure_ctrl().await?;
        write_msg(
            &mut *ch.write.lock().await,
            &RelayMsg {
                kind: Some(Kind::PunchAck(ack)),
            },
        )
        .await
        .map_err(RelayError::from)
    }

    /// 下一条异步事件（入站 PunchReq / PunchAck / 控制链路关闭）。
    pub async fn next_event(&mut self) -> Option<RelayEvent> {
        self.events_rx.recv().await
    }

    async fn control_roundtrip(
        &mut self,
        msg: RelayMsg,
        what: &'static str,
    ) -> Result<RelayMsg, RelayError> {
        let ch = self.ensure_ctrl().await?;
        let (tx, rx) = oneshot::channel();
        *ch.pending.lock().await = Some(tx);
        write_msg(&mut *ch.write.lock().await, &msg).await?;
        let reply = tokio::time::timeout(REPLY_TIMEOUT, rx).await.map_err(|_| {
            tracing::warn!(waiting = what, "control roundtrip timed out");
            RelayError::Timeout(what)
        })?;
        reply.map_err(|_| RelayError::LinkClosed)
    }

    async fn ensure_ctrl(&mut self) -> Result<&CtrlChannel, RelayError> {
        if self.ctrl.is_none() {
            let stream = self.link.open_stream().await?;
            let (rh, wh) = split(stream);
            let pending: PendingSlot = Arc::new(Mutex::new(None));
            tokio::spawn(read_ctrl_loop(rh, pending.clone(), self.events_tx.clone()));
            let write: Arc<CtrlWrite> = Arc::new(Mutex::new(wh));
            self.ctrl = Some(CtrlChannel { write, pending });
        }
        Ok(self.ctrl.as_ref().expect("control channel just set"))
    }
}

async fn read_ctrl_loop(
    mut rh: ReadHalf<BoxedStream>,
    pending: PendingSlot,
    events: mpsc::Sender<RelayEvent>,
) {
    loop {
        match read_msg(&mut rh).await {
            Ok(Some(msg)) => dispatch_ctrl(&pending, &events, msg).await,
            Ok(None) => break,
            Err(e) => {
                tracing::warn!(error = %e, "control stream read failed");
                break;
            }
        }
    }
    if let Some(tx) = pending.lock().await.take() {
        let _ = tx.send(RelayMsg::error(errcode::PROTOCOL, "control stream closed"));
    }
    let _ = events.send(RelayEvent::ControlClosed).await;
}

async fn dispatch_ctrl(pending: &PendingSlot, events: &mpsc::Sender<RelayEvent>, msg: RelayMsg) {
    let reply_shaped = matches!(
        msg.kind,
        Some(Kind::Reserved(_)) | Some(Kind::PunchAck(_)) | Some(Kind::Reject(_))
    );
    if reply_shaped {
        if let Some(tx) = pending.lock().await.take() {
            let _ = tx.send(msg);
            return;
        }
    }
    match msg.kind {
        Some(Kind::PunchReq(p)) => push_event(events, RelayEvent::PunchReq(p)).await,
        Some(Kind::PunchAck(a)) => push_event(events, RelayEvent::PunchAck(a)).await,
        Some(Kind::Reserved(_)) | Some(Kind::Reject(_)) => {
            tracing::warn!("control reply arrived with no pending request; dropped");
        }
        other => tracing::warn!(kind = ?other, "unexpected frame on control stream; ignored"),
    }
}

async fn push_event(events: &mpsc::Sender<RelayEvent>, ev: RelayEvent) {
    if events.try_send(ev).is_err() {
        tracing::warn!("event queue full; punch event dropped");
    }
}

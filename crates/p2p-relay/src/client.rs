//! RelayClient：reserve/connect 状态机 + Punch 信令收发，对 RelayLink 接缝编程。
//!
//! 状态机：reserve 发 Reserve 收 Reserved 发放 CircuitId；connect 发 Connect 收
//! Bound 后该流即电路对端；punch 走控制流，入站信号经事件队列交付调用方。
//! E6：控制流建立即启动保活任务，连续超时判定失联并上抛事件（见 keepalive 模块）。

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use p2p_mux::BoxedStream;
use tokio::io::{split, ReadHalf};
use tokio::sync::{mpsc, Mutex};

use crate::error::{error_from_wire, RelayError};
use crate::frame::{read_msg, write_msg};
use crate::keepalive::{
    roundtrip, spawn_keepalive, CtrlInner, RelayKeepalive, ReplyExpect, RoundtripLock,
};
use crate::link::RelayLink;
use crate::messages::{errcode, relay_msg::Kind, PunchAck, PunchReq, RelayMsg};
use crate::CircuitId;

/// 控制请求回包超时。
const REPLY_TIMEOUT: Duration = Duration::from_secs(5);
/// 事件队列容量；满则丢事件并留日志（不反压控制流）。
const EVENT_CAPACITY: usize = 32;

/// 异步事件：入站打洞信令与控制链路关闭（关闭必带原因，E5 原因链）。
/// E6 起保活判失联同样经 ControlClosed 上抛，reason 带 keepalive 归因。
#[derive(Debug)]
pub enum RelayEvent {
    PunchReq(PunchReq),
    PunchAck(PunchAck),
    ControlClosed { reason: String },
}

struct CtrlChannel {
    inner: Arc<CtrlInner>,
    lock: RoundtripLock,
    /// 保活任务句柄：client drop 时中止，防写半被任务钉住、服务端收不到 EOF。
    keepalive: tokio::task::JoinHandle<()>,
}

/// 中继客户端；一个实例对应一条到 relay 的链路。
pub struct RelayClient {
    link: Box<dyn RelayLink>,
    ctrl: Option<CtrlChannel>,
    events_tx: mpsc::Sender<RelayEvent>,
    events_rx: mpsc::Receiver<RelayEvent>,
    keepalive: RelayKeepalive,
}

impl Drop for RelayClient {
    fn drop(&mut self) {
        if let Some(ch) = self.ctrl.take() {
            ch.keepalive.abort();
        }
    }
}

impl RelayClient {
    pub fn new(link: Box<dyn RelayLink>) -> Self {
        Self::with_keepalive(link, RelayKeepalive::default())
    }

    /// 指定保活参数的客户端；间隔/超时/失联阈值见 [RelayKeepalive]。
    pub fn with_keepalive(link: Box<dyn RelayLink>, keepalive: RelayKeepalive) -> Self {
        let (events_tx, events_rx) = mpsc::channel(EVENT_CAPACITY);
        Self {
            link,
            ctrl: None,
            events_tx,
            events_rx,
            keepalive,
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
                ReplyExpect::Reserved,
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
            .control_roundtrip(
                RelayMsg::punch_req(target, addrs),
                "punch-ack",
                ReplyExpect::PunchAck,
            )
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
            &mut *ch.inner.write.lock().await,
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
        expect: ReplyExpect,
    ) -> Result<RelayMsg, RelayError> {
        let ch = self.ensure_ctrl().await?;
        roundtrip(&ch.inner, &ch.lock, msg, expect, what, REPLY_TIMEOUT).await
    }

    async fn ensure_ctrl(&mut self) -> Result<&CtrlChannel, RelayError> {
        if self.ctrl.is_none() {
            let stream = self.link.open_stream().await?;
            let (rh, wh) = split(stream);
            let inner = Arc::new(CtrlInner {
                write: Arc::new(Mutex::new(wh)),
                pending: Arc::new(Mutex::new(None)),
                lost: AtomicBool::new(false),
                closed: AtomicBool::new(false),
            });
            let lock: RoundtripLock = Arc::new(Mutex::new(()));
            let task = spawn_keepalive(
                inner.clone(),
                lock.clone(),
                self.events_tx.clone(),
                self.keepalive.clone(),
                self.link.peer_id().to_string(),
            );
            tokio::spawn(read_ctrl_loop(rh, inner.clone(), self.events_tx.clone()));
            self.ctrl = Some(CtrlChannel {
                inner,
                lock,
                keepalive: task,
            });
        }
        Ok(self.ctrl.as_ref().expect("control channel just set"))
    }
}

async fn read_ctrl_loop(
    mut rh: ReadHalf<BoxedStream>,
    inner: Arc<CtrlInner>,
    events: mpsc::Sender<RelayEvent>,
) {
    let reason;
    loop {
        match read_msg(&mut rh).await {
            Ok(Some(msg)) => dispatch_ctrl(&inner, &events, msg).await,
            Ok(None) => {
                reason = "control stream eof (clean close by peer)".to_string();
                break;
            }
            Err(e) => {
                tracing::warn!(error = %e, "control stream read failed");
                reason = e.to_string();
                break;
            }
        }
    }
    inner.closed.store(true, Ordering::Relaxed);
    if let Some(tx) = inner.pending.lock().await.take() {
        let _ = tx
            .tx
            .send(RelayMsg::error(errcode::PROTOCOL, "control stream closed"));
    }
    let _ = events.send(RelayEvent::ControlClosed { reason }).await;
}

/// 控制帧分发：回包投给形态匹配的待应答者，信令进事件队列，违规留告警。
async fn dispatch_ctrl(inner: &Arc<CtrlInner>, events: &mpsc::Sender<RelayEvent>, msg: RelayMsg) {
    let Some(kind) = msg.kind else {
        tracing::warn!("empty control frame; ignored");
        return;
    };
    let reply_shaped = matches!(
        kind,
        Kind::Reserved(_) | Kind::PunchAck(_) | Kind::KeepAliveAck(_) | Kind::Reject(_)
    );
    if reply_shaped {
        let mut slot = inner.pending.lock().await;
        if slot.as_ref().is_some_and(|p| p.matches(&kind)) {
            if let Some(p) = slot.take() {
                drop(slot);
                let _ = p.tx.send(RelayMsg { kind: Some(kind) });
                return;
            }
        }
        drop(slot);
    }
    match kind {
        Kind::PunchReq(p) => push_event(events, RelayEvent::PunchReq(p)).await,
        Kind::PunchAck(a) => push_event(events, RelayEvent::PunchAck(a)).await,
        Kind::Reserved(_) | Kind::Reject(_) | Kind::KeepAliveAck(_) => {
            tracing::warn!("control reply arrived with no matching pending request; dropped");
        }
        other => tracing::warn!(kind = ?other, "unexpected frame on control stream; ignored"),
    }
}

async fn push_event(events: &mpsc::Sender<RelayEvent>, ev: RelayEvent) {
    if events.try_send(ev).is_err() {
        tracing::warn!("event queue full; punch event dropped");
    }
}

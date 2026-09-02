//! 控制面：Reserve 发放 + Punch 信令转发；协议违规即断控制流（design 6）。

use std::io;
use std::sync::Arc;

use p2p_mux::BoxedStream;
use tokio::io::{split, ReadHalf};

use crate::frame::{read_msg, write_msg};
use crate::messages::{errcode, relay_msg::Kind, RelayMsg, Reserve};
use crate::service::RelayServiceImpl;
use crate::state::CtrlWrite;

impl RelayServiceImpl {
    /// Reserve 首帧已读：拆流、登记控制写半、回 Reserved 后进入控制循环。
    pub(crate) async fn handle_control(
        self: Arc<Self>,
        peer: String,
        stream: BoxedStream,
        first: Reserve,
    ) {
        let (mut rh, wh) = split(stream);
        let write: Arc<CtrlWrite> = Arc::new(tokio::sync::Mutex::new(wh));
        self.lock_state().register_control(&peer, write.clone());
        let reply = self.on_reserve(&peer, first).await;
        if send_ctrl(&write, reply).await.is_err() {
            tracing::warn!(peer = %peer, "reserved reply write failed; control stream cut");
            self.lock_state().remove_control_if(&peer, &write);
            return;
        }
        self.control_loop(&peer, &mut rh, &write).await;
        self.lock_state().remove_control_if(&peer, &write);
        tracing::info!(peer = %peer, "control stream closed");
    }

    async fn control_loop(
        &self,
        peer: &str,
        rh: &mut ReadHalf<BoxedStream>,
        write: &Arc<CtrlWrite>,
    ) {
        loop {
            match read_msg(rh).await {
                Ok(Some(msg)) => {
                    if !self.dispatch_control(peer, msg, write).await {
                        break;
                    }
                }
                Ok(None) => break,
                Err(e) => {
                    tracing::warn!(peer = %peer, error = %e, "control read failed; cutting");
                    break;
                }
            }
        }
    }

    /// 处理一帧控制消息；返回 false 表示协议违规需断流。
    async fn dispatch_control(&self, peer: &str, msg: RelayMsg, write: &Arc<CtrlWrite>) -> bool {
        match msg.kind {
            Some(Kind::Reserve(r)) => send_ctrl(write, self.on_reserve(peer, r).await)
                .await
                .is_ok(),
            Some(Kind::PunchReq(p)) => {
                self.forward_punch(peer, write, p.peer_id, p.addrs, true)
                    .await
            }
            Some(Kind::PunchAck(a)) => {
                self.forward_punch(peer, write, a.peer_id, a.addrs, false)
                    .await
            }
            other => {
                tracing::warn!(peer = %peer, kind = ?other, "protocol violation on control stream; cutting");
                let _ = send_ctrl(
                    write,
                    RelayMsg::error(errcode::PROTOCOL, "unexpected frame on control stream"),
                )
                .await;
                false
            }
        }
    }

    async fn on_reserve(&self, peer: &str, r: Reserve) -> RelayMsg {
        let issued = self.lock_state().issue_circuit(
            peer,
            &r.allowed_joiner,
            r.ttl_secs,
            self.limits().max_circuits_per_peer,
            self.limits().max_total_circuits,
        );
        match issued {
            Ok(cid) => {
                tracing::info!(
                    peer = %peer,
                    circuit = cid,
                    ttl_secs = r.ttl_secs,
                    joiner = %r.allowed_joiner,
                    "circuit reserved"
                );
                RelayMsg::reserved(cid)
            }
            Err(errcode::PEER_LIMIT) => {
                tracing::warn!(peer = %peer, "reserve rejected: per-peer circuit quota");
                RelayMsg::error(errcode::PEER_LIMIT, "per-peer circuit quota exceeded")
            }
            Err(errcode::GLOBAL_CAPACITY) => {
                tracing::warn!(peer = %peer, "reserve rejected: global circuit capacity exhausted");
                RelayMsg::error(
                    errcode::GLOBAL_CAPACITY,
                    "global circuit capacity exhausted",
                )
            }
            Err(code) => {
                tracing::warn!(peer = %peer, code, "reserve rejected");
                RelayMsg::error(code, "circuit reserve rejected")
            }
        }
    }

    /// 转发打洞信令：peer_id 改写为发送方；目标不在线时给请求方显式拒绝。
    async fn forward_punch(
        &self,
        sender: &str,
        write: &Arc<CtrlWrite>,
        target: String,
        addrs: Vec<String>,
        is_req: bool,
    ) -> bool {
        let frame = if is_req {
            RelayMsg::punch_req(sender, addrs)
        } else {
            RelayMsg::punch_ack(sender, addrs)
        };
        let Some(dest) = self.lock_state().control_of(&target) else {
            tracing::warn!(from = %sender, to = %target, "punch target has no control link");
            let _ = send_ctrl(
                write,
                RelayMsg::error(
                    errcode::PUNCH_TARGET_UNKNOWN,
                    format!("peer {target} offline"),
                ),
            )
            .await;
            return true;
        };
        if let Err(e) = send_ctrl(&dest, frame).await {
            tracing::warn!(from = %sender, to = %target, error = %e, "punch forward failed; target link broken");
        } else {
            tracing::info!(from = %sender, to = %target, is_req, "punch signal forwarded");
        }
        true
    }
}

/// 经登记的控制写半发一帧。
async fn send_ctrl(w: &Arc<CtrlWrite>, msg: RelayMsg) -> io::Result<()> {
    write_msg(&mut *w.lock().await, &msg).await
}

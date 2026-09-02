//! 打洞协调状态机（design 7.3 步骤 2）：本轮只保证信令时序闭环，真实探测属 M4。
//!
//! 时序：主动方 Idle -> RequestSent ->（收 Ack）-> Probing；
//! 被动方 Idle ->（收 Req 构造应答）-> AckSent ->（回包完成）-> Probing。
//! 两侧进入 Probing 即为「同时探测」时点，探测定向地址取信令携带的 addrs。

use crate::error::RelayError;
use crate::messages::{PunchAck, PunchReq};

/// 打洞会话阶段。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PunchPhase {
    /// 尚未发出或收到任何信令。
    Idle,
    /// 已向对端发出 PunchReq，等待 PunchAck。
    RequestSent,
    /// 已收到 PunchReq，应答待发或已发。
    AckSent,
    /// 信令闭环：双方此刻同时开洞探测。
    Probing,
}

pub struct PunchSession {
    remote: String,
    phase: PunchPhase,
}

impl PunchSession {
    /// 主动方会话：准备向 remote 发 PunchReq。
    pub fn initiator(remote: impl Into<String>) -> Self {
        Self { remote: remote.into(), phase: PunchPhase::Idle }
    }

    /// 被动方会话：由入站 PunchReq 构造（remote = 请求方）。
    pub fn responder(req: &PunchReq) -> Self {
        Self { remote: req.peer_id.clone(), phase: PunchPhase::AckSent }
    }

    pub fn phase(&self) -> &PunchPhase {
        &self.phase
    }

    pub fn remote(&self) -> &str {
        &self.remote
    }

    /// 主动方：PunchReq 已发出。
    pub fn mark_request_sent(&mut self) -> Result<(), RelayError> {
        if self.phase != PunchPhase::Idle {
            return Err(RelayError::Protocol(format!("punch {:?}: illegal mark_request_sent", self.phase)));
        }
        self.phase = PunchPhase::RequestSent;
        Ok(())
    }

    /// 主动方：收到 PunchAck，双方同时进入探测时点。
    pub fn on_ack(&mut self) -> Result<(), RelayError> {
        if self.phase != PunchPhase::RequestSent {
            return Err(RelayError::Protocol(format!("punch {:?}: illegal on_ack", self.phase)));
        }
        self.phase = PunchPhase::Probing;
        Ok(())
    }

    /// 被动方：构造应答（peer_id = 请求方，relay 转发时改写）。
    pub fn build_ack(&self, local_addrs: Vec<String>) -> PunchAck {
        PunchAck { peer_id: self.remote.clone(), addrs: local_addrs }
    }

    /// 被动方：应答已发出，双方同时进入探测时点。
    pub fn mark_probing(&mut self) -> Result<(), RelayError> {
        if self.phase != PunchPhase::AckSent {
            return Err(RelayError::Protocol(format!("punch {:?}: illegal mark_probing", self.phase)));
        }
        self.phase = PunchPhase::Probing;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn req_from_a() -> PunchReq {
        PunchReq { peer_id: "peer-a".into(), addrs: vec!["10.0.0.1:4001".into()] }
    }

    #[test]
    fn initiator_and_responder_reach_probing() {
        // 主动方 A
        let mut a = PunchSession::initiator("peer-b");
        assert_eq!(a.remote(), "peer-b");
        a.mark_request_sent().unwrap();
        assert_eq!(a.phase(), &PunchPhase::RequestSent);

        // 被动方 B：收到经 relay 改写的 PunchReq（peer_id = 发送方）
        let mut b = PunchSession::responder(&req_from_a());
        assert_eq!(b.remote(), "peer-a");
        let ack = b.build_ack(vec!["10.0.0.2:4002".into()]);
        assert_eq!(ack.peer_id, "peer-a");
        b.mark_probing().unwrap();
        assert_eq!(b.phase(), &PunchPhase::Probing);

        // A 收到经 relay 改写的 PunchAck（peer_id = 应答方）
        let ack_at_a = PunchAck { peer_id: "peer-b".into(), addrs: ack.addrs };
        let _ = ack_at_a;
        a.on_ack().unwrap();
        assert_eq!(a.phase(), &PunchPhase::Probing);
    }

    #[test]
    fn illegal_transitions_rejected() {
        let mut a = PunchSession::initiator("x");
        assert!(a.on_ack().is_err(), "ack before request must fail");
        a.mark_request_sent().unwrap();
        assert!(a.mark_request_sent().is_err(), "double request must fail");

        let mut b = PunchSession::responder(&req_from_a());
        assert!(b.mark_request_sent().is_err(), "responder cannot send request");
        b.mark_probing().unwrap();
        assert!(b.mark_probing().is_err(), "double probing transition must fail");
    }
}

//! gate 单测（Amended A-1 红绿锚点）：判定次序 send→attachment、短路、缺省放行、
//! 四因 reason 原样上浮（chat 侧不解释 reason，只决定拒收）。

use std::sync::Mutex;

use crate::gate::{admit_message, CheckGate, Gate, Reject};
use p2p_identity::PeerId;

/// 记录调用序的可编程假闸：deny_at 指定在哪一闸拒（None=恒 Allow）。
#[derive(Default)]
struct Recording {
    calls: Mutex<Vec<bool>>,
    deny_at: Option<bool>,
    reason: String,
}

impl Recording {
    fn allowing() -> Self {
        Self::default()
    }

    fn denying_at(deny_at: bool, reason: &str) -> Self {
        Self {
            deny_at: Some(deny_at),
            reason: reason.to_owned(),
            ..Self::default()
        }
    }

    fn recorded(&self) -> Vec<bool> {
        self.calls.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }
}

impl CheckGate for Recording {
    fn admit(&self, _peer: &PeerId, media: bool) -> Result<(), Reject> {
        self.calls
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push(media);
        if self.deny_at == Some(media) {
            return Err(Reject {
                reason: self.reason.clone(),
            });
        }
        Ok(())
    }
}

fn peer() -> PeerId {
    PeerId::from_bytes([7u8; 32])
}

fn gate(g: Recording) -> (Gate, std::sync::Arc<Recording>) {
    let arc = std::sync::Arc::new(g);
    (Some(arc.clone()), arc)
}

#[test]
fn no_gate_allows_without_calls() {
    assert!(admit_message(&None, &peer(), true).is_ok());
}

#[test]
fn text_message_hits_send_gate_only() {
    let (g, rec) = gate(Recording::allowing());
    admit_message(&g, &peer(), false).unwrap();
    assert_eq!(rec.recorded(), vec![false]);
}

#[test]
fn media_message_hits_send_then_attachment_gates() {
    let (g, rec) = gate(Recording::allowing());
    admit_message(&g, &peer(), true).unwrap();
    assert_eq!(rec.recorded(), vec![false, true]);
}

#[test]
fn send_deny_short_circuits_attachment_gate() {
    let (g, rec) = gate(Recording::denying_at(false, "NotBound"));
    let err = admit_message(&g, &peer(), true).unwrap_err();
    assert_eq!(err.reason, "NotBound");
    assert_eq!(rec.recorded(), vec![false], "attachment 闸不得被调用");
}

#[test]
fn attachment_deny_rejects_whole_frame() {
    let (g, rec) = gate(Recording::denying_at(true, "MissingPerm"));
    let err = admit_message(&g, &peer(), true).unwrap_err();
    assert_eq!(err.reason, "MissingPerm");
    assert_eq!(rec.recorded(), vec![false, true]);
}

/// A-1 四因全适用 + 读失败：chat 侧对 reason 码不解释，一律整帧拒收。
#[test]
fn every_deny_reason_shape_rejects() {
    for reason in [
        "NotBound",
        "Expired",
        "BrokenRole",
        "MissingPerm",
        "ReadFailed",
    ] {
        let (g, _rec) = gate(Recording::denying_at(false, reason));
        let err = admit_message(&g, &peer(), false).unwrap_err();
        assert_eq!(err.reason, reason, "reason 码必须原样上浮");
    }
}

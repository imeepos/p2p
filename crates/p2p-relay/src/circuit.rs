//! 电路面：Connect 流配额检查、同号配对与 copy_bidirectional 密文桥接。

use std::sync::Arc;

use p2p_mux::BoxedStream;
use tokio::io::copy_bidirectional;

use crate::limits::RateLimitedStream;
use crate::messages::{errcode, write_msg, write_reject, RelayMsg};
use crate::service::RelayServiceImpl;
use crate::slots::{CircuitOutcome, PendingStream};

impl RelayServiceImpl {
    pub(crate) async fn handle_connect(
        self: Arc<Self>,
        joiner: String,
        stream: BoxedStream,
        cid: u64,
    ) {
        let outcome = {
            let mut st = self.lock_state();
            st.on_connect(&joiner, cid, self.limits().max_circuits_per_peer, stream)
        };
        match outcome {
            CircuitOutcome::Parked => {
                tracing::debug!(peer = %joiner, circuit = cid, "circuit half parked; waiting for peer");
            }
            CircuitOutcome::Paired(pending, stream) => {
                self.bridge(cid, pending, joiner, stream).await
            }
            CircuitOutcome::Rejected(code, message, mut stream) => {
                tracing::warn!(peer = %joiner, circuit = cid, code, "connect rejected");
                let _ = write_reject(&mut stream, code, message).await;
            }
        }
    }

    /// 两侧都只是密文字节流：relay 不解析内容，限速按各自出口方向计数。
    async fn bridge(
        self: Arc<Self>,
        cid: u64,
        pending: PendingStream,
        joiner: String,
        mut stream: BoxedStream,
    ) {
        // 先向两侧各发 Bound（客户端 connect 依赖它返回），任一侧失败即取消桥接
        let mut parked = pending.stream;
        if let Err(e) = write_msg(&mut parked, &RelayMsg::bound(cid)).await {
            tracing::warn!(circuit = cid, holder = %pending.peer, error = %e, "bound write failed; bridge cancelled");
            let _ = write_reject(&mut stream, errcode::PROTOCOL, "circuit peer vanished").await;
            self.release_two(&pending.peer, &joiner);
            return;
        }
        if let Err(e) = write_msg(&mut stream, &RelayMsg::bound(cid)).await {
            tracing::warn!(circuit = cid, joiner = %joiner, error = %e, "bound write failed; bridge cancelled");
            self.release_two(&pending.peer, &joiner);
            return;
        }
        let mut a = RateLimitedStream::new(parked, self.bucket_for(&pending.peer));
        let mut b = RateLimitedStream::new(stream, self.bucket_for(&joiner));
        tracing::info!(circuit = cid, a = %pending.peer, b = %joiner, "circuit bridged");
        match copy_bidirectional(&mut a, &mut b).await {
            Ok((a_to_b, b_to_a)) => {
                tracing::info!(circuit = cid, a_to_b, b_to_a, "circuit closed cleanly")
            }
            Err(e) => tracing::warn!(circuit = cid, error = %e, "circuit aborted"),
        }
        self.release_two(&pending.peer, &joiner);
    }

    fn release_two(&self, peer_a: &str, peer_b: &str) {
        let mut st = self.lock_state();
        st.release_circuit_load(peer_a);
        st.release_circuit_load(peer_b);
    }
}

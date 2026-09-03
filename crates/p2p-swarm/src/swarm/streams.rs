//! 业务开流路径（E8 自 mod.rs 迁出）：按需取/建连接 + 使用度记账。
//! 探活 ping（ping.rs probe_once）直开 mux 流、不经此处：底座维持流量
//! 不计入「使用中」，否则空闲回收永不触发（见 usage.rs 模块注释）。

use std::io;

use p2p_identity::PeerId;
use p2p_mux::BoxedStream;
use p2p_protocol::ProtocolId;

use super::dial::dial_peer;
use super::Swarm;
use crate::liveness::LivenessSource;
use crate::usage::{unix_now, TrackedStream};

impl Swarm {
    /// 开裸流（协议 ID 首帧由调用方写入）：按需取/建连接。
    /// 开流计入在途与最后使用（空闲回收的使用中豁免依据）；
    /// 失败路径守护随作用域析构归还计数，不静默。
    pub async fn open_stream(
        &self,
        peer: &PeerId,
        _protocol: &ProtocolId,
    ) -> io::Result<BoxedStream> {
        let mux = self.pool.get_or_dial(*peer, dial_peer(self, *peer)).await?;
        let Some(usage) = self.pool.usage(peer) else {
            // get_or_dial 成功即入池；记账缺失属实现缺陷，显式报错不静默
            return Err(io::Error::other(
                "pooled connection lacks usage bookkeeping",
            ));
        };
        let guard = usage.enter();
        match mux.open_stream().await {
            Ok(stream) => {
                // E8：开流成功 = 对端在网（活跃度正信号）+ 使用记账（豁免回收）
                usage.touch(unix_now());
                self.liveness
                    .note_alive(*peer, LivenessSource::Connection, unix_now());
                Ok(Box::new(TrackedStream::new(stream, guard)))
            }
            Err(err) => Err(err),
        }
    }
}

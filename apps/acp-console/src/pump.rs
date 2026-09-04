//! WS⇄P2P 纯字节泵（需求 B）：双向有界对拷，不解析 ACP 语义。
//! 有界性：P2P 读写均按固定块推进，WS 读侧消息上限由 accept 配置（16 MiB，
//! 对齐 acp-common 单行护栏）；任一侧断开即终止另一侧（连接关闭双向传播）。
//! 底座 yamux 窗口更新为批量策略（<半窗不发），极端时序下写侧唤醒可能丢失，
//! 故读写均带超时重 kick：停滞即告警日志 + 重新 poll，禁止无限静默悬挂。

use std::io;
use std::time::Duration;

use futures_util::stream::{SplitSink, SplitStream};
use futures_util::{SinkExt, StreamExt};
use p2p::BoxedStream;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio_tungstenite::tungstenite::protocol::frame::coding::CloseCode;
use tokio_tungstenite::tungstenite::protocol::{CloseFrame, Message};
use tokio_tungstenite::tungstenite::Bytes;
use tokio_tungstenite::WebSocketStream;

/// P2P 单次读写块上限：有界缓冲，写侧背压由 WS send 的 await 语义提供。
const CHUNK: usize = 64 * 1024;
/// WS→P2P 分块写粒度：与底座 yamux split_send_size 对齐，减少大帧排队。
const WRITE_CHUNK: usize = 16 * 1024;
/// 写停滞重 kick 护栏。
const WRITE_GRACE: Duration = Duration::from_secs(5);
/// 读停滞重 kick 护栏（空闲连接合法，护栏须远大于正常静默间隔）。
const READ_GRACE: Duration = Duration::from_secs(30);

type Ws = WebSocketStream<TcpStream>;
type WsSink = SplitSink<Ws, Message>;
type WsStream = SplitStream<Ws>;
type PeerRead = tokio::io::ReadHalf<BoxedStream>;
type PeerWrite = tokio::io::WriteHalf<BoxedStream>;

/// 泵结束原因：区分对端断（agent 侧）与客户端断（WS 侧），失败带原因串。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PumpEnd {
    PeerClosed,
    ClientClosed,
    Failed,
}

pub async fn run(stream: BoxedStream, sink: WsSink, inbound: WsStream) -> PumpEnd {
    let (peer_read, peer_write) = tokio::io::split(stream);
    let mut to_ws = tokio::spawn(pump_p2p_to_ws(peer_read, sink));
    let mut to_peer = tokio::spawn(pump_ws_to_p2p(inbound, peer_write));
    // 首侧结束即取消另一侧：双向 join 会因对侧永久 pending 而悬挂
    // （如 p2p 断流后 WS 侧无消息可读）。取消安全：连接已死，无优雅收尾可丢。
    let end = tokio::select! {
        end = &mut to_ws => join_result(end),
        end = &mut to_peer => join_result(end),
    };
    to_ws.abort();
    to_peer.abort();
    end
}

fn join_result(res: Result<PumpEnd, tokio::task::JoinError>) -> PumpEnd {
    res.unwrap_or_else(|e| {
        tracing::warn!(error = %e, "pump task aborted or panicked");
        PumpEnd::Failed
    })
}

/// P2P → WS：读 64 KiB 块逐块转 Binary 帧；对端 EOF → 通知 WS 干净关闭。
async fn pump_p2p_to_ws(mut peer_read: PeerRead, mut sink: WsSink) -> PumpEnd {
    loop {
        let mut buf = vec![0u8; CHUNK];
        let n = match read_kick(&mut peer_read, &mut buf).await {
            Ok(0) => break,
            Ok(n) => n,
            Err(e) => {
                tracing::warn!(error = %e, "pump: p2p read failed");
                return PumpEnd::Failed;
            }
        };
        if sink
            .send(Message::Binary(Bytes::copy_from_slice(&buf[..n])))
            .await
            .is_err()
        {
            tracing::info!("pump: ws sink closed while forwarding");
            return PumpEnd::ClientClosed;
        }
    }
    let close = CloseFrame {
        code: CloseCode::Normal,
        reason: "peer closed".into(),
    };
    let _ = sink.send(Message::Close(Some(close))).await;
    let _ = sink.close().await;
    PumpEnd::PeerClosed
}

/// WS → P2P：Binary/Text 帧按原始字节透传；Close/EOF → 关 P2P 写半（EOF 传播）。
async fn pump_ws_to_p2p(mut inbound: WsStream, mut peer_write: PeerWrite) -> PumpEnd {
    while let Some(item) = inbound.next().await {
        match item {
            Ok(msg @ (Message::Binary(_) | Message::Text(_))) => {
                let data = msg.into_data();
                if let Err(e) = write_kick(&mut peer_write, &data).await {
                    tracing::warn!(error = %e, "pump: p2p write failed");
                    return PumpEnd::Failed;
                }
            }
            Ok(Message::Close(_)) => {
                let _ = peer_write.shutdown().await;
                return PumpEnd::ClientClosed;
            }
            Ok(_) => {}
            Err(e) => {
                tracing::warn!(error = %e, "pump: ws read failed");
                let _ = peer_write.shutdown().await;
                return PumpEnd::Failed;
            }
        }
    }
    let _ = peer_write.shutdown().await;
    PumpEnd::ClientClosed
}

/// 分块写 + 停滞重 kick：块间显式 flush，停滞告警后重新 poll（唤醒丢失兜底）。
async fn write_kick(peer_write: &mut PeerWrite, mut data: &[u8]) -> io::Result<()> {
    while !data.is_empty() {
        let len = data.len().min(WRITE_CHUNK);
        match tokio::time::timeout(WRITE_GRACE, peer_write.write(&data[..len])).await {
            Ok(Ok(0)) => {
                return Err(io::Error::new(
                    io::ErrorKind::WriteZero,
                    "p2p write made no progress",
                ));
            }
            Ok(Ok(n)) => data = &data[n..],
            Ok(Err(e)) => return Err(e),
            Err(_) => {
                tracing::warn!(pending = data.len(), "pump: p2p write stalled; re-kick");
            }
        }
    }
    loop {
        match tokio::time::timeout(WRITE_GRACE, peer_write.flush()).await {
            Ok(Ok(())) => return Ok(()),
            Ok(Err(e)) => return Err(e),
            Err(_) => tracing::warn!("pump: p2p flush stalled; re-kick"),
        }
    }
}

/// 读 + 停滞重 kick：护栏超时告警后重新 poll（唤醒丢失兜底），不把超时当 EOF。
async fn read_kick(peer_read: &mut PeerRead, buf: &mut [u8]) -> io::Result<usize> {
    loop {
        match tokio::time::timeout(READ_GRACE, peer_read.read(buf)).await {
            Ok(res) => return res,
            Err(_) => tracing::warn!(grace = ?READ_GRACE, "pump: p2p read stalled; re-kick"),
        }
    }
}

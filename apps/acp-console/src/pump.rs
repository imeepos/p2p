//! WS⇄P2P 帧化泵（需求 B）：双向有界对拷，不解析 ACP 语义。
//! wire 帧面（设计 §4.2-1）：P2P 侧一律 varint 长度前缀帧——WS 消息字节经
//! acp-common frames() 切帧下发、P2P 帧经 LineReassembler 重组为行上行；
//! 裸透传会被 agent read_wire_line 当作帧长解析（历史缺陷，2026-09-08 修复）。
//! 有界性：P2P 单帧 ≤ 1 MiB（p2p-protocol 护栏）、单行 ≤ 16 MiB（acp-common
//! 护栏，对齐 WS 读侧消息上限）；任一侧断开即终止另一侧（双向传播）。
//! 底座 yamux 窗口更新为批量策略（<半窗不发），极端时序下写侧唤醒可能丢失，
//! 故读写均带超时重 kick：停滞即告警日志 + 重新 poll，禁止无限静默悬挂。

use std::io;
use std::time::Duration;

use acp_common::LineReassembler;
use futures_util::stream::{SplitSink, SplitStream};
use futures_util::{SinkExt, StreamExt};
use p2p::BoxedStream;
use p2p_protocol::{read_frame, write_frame};
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;
use tokio_tungstenite::tungstenite::protocol::frame::coding::CloseCode;
use tokio_tungstenite::tungstenite::protocol::{CloseFrame, Message};
use tokio_tungstenite::WebSocketStream;

/// 单帧载荷上限：与 p2p-protocol MAX_FRAME_SIZE 对齐（1 MiB）。
const MAX_PAYLOAD: usize = 1024 * 1024;
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

/// P2P → WS：varint 帧读 + 行重组，逐行转 Text 消息；对端 EOF → 通知 WS 干净关闭。
async fn pump_p2p_to_ws(mut peer_read: PeerRead, mut sink: WsSink) -> PumpEnd {
    let mut reassembler = LineReassembler::new();
    loop {
        let frame = match read_frame_kick(&mut peer_read).await {
            Ok(Some(frame)) => frame,
            Ok(None) => break,
            Err(e) => {
                tracing::warn!(error = %e, "pump: p2p read failed");
                return PumpEnd::Failed;
            }
        };
        if let Err(e) = reassembler.push_frame(&frame) {
            tracing::warn!(error = %e, "pump: p2p line reassembly failed");
            return PumpEnd::Failed;
        }
        while let Some(line) = reassembler.take_line() {
            let text = String::from_utf8_lossy(&line).into_owned();
            if sink.send(Message::Text(text.into())).await.is_err() {
                tracing::info!("pump: ws sink closed while forwarding");
                return PumpEnd::ClientClosed;
            }
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

/// WS → P2P：Binary/Text 消息字节经 frames() 切帧下发；Close/EOF → 关 P2P 写半（EOF 传播）。
async fn pump_ws_to_p2p(mut inbound: WsStream, mut peer_write: PeerWrite) -> PumpEnd {
    while let Some(item) = inbound.next().await {
        match item {
            Ok(msg @ (Message::Binary(_) | Message::Text(_))) => {
                let data = msg.into_data();
                // 纯载荷分帧：WS 字节原样成帧（帧内不注入换行），换行语义归
                // 上游字节本身；行 > 1 MiB 由 GUI/agent 侧行护栏约束。
                for chunk in data.chunks(MAX_PAYLOAD) {
                    if let Err(e) = write_frame_kick(&mut peer_write, chunk).await {
                        tracing::warn!(error = %e, "pump: p2p write failed");
                        return PumpEnd::Failed;
                    }
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

/// 单帧写 + 停滞重 kick：写停滞告警后重新 poll（唤醒丢失兜底），不把超时当失败。
async fn write_frame_kick(peer_write: &mut PeerWrite, frame: &[u8]) -> io::Result<()> {
    loop {
        match tokio::time::timeout(WRITE_GRACE, write_frame(peer_write, frame)).await {
            Ok(Ok(())) => return Ok(()),
            Ok(Err(e)) => return Err(e),
            Err(_) => tracing::warn!(len = frame.len(), "pump: p2p write stalled; re-kick"),
        }
    }
}

/// 单帧读 + 停滞重 kick：护栏超时告警后重新 poll（唤醒丢失兜底），不把超时当 EOF。
/// 干净 EOF（对端关流）返回 None；kick 重试语义与既有 read_kick 一致（停滞=无进度）。
async fn read_frame_kick(peer_read: &mut PeerRead) -> io::Result<Option<Vec<u8>>> {
    loop {
        match tokio::time::timeout(READ_GRACE, read_frame(peer_read)).await {
            Ok(Ok(frame)) => return Ok(Some(frame)),
            Ok(Err(e)) if e.kind() == io::ErrorKind::UnexpectedEof => return Ok(None),
            Ok(Err(e)) => return Err(e),
            Err(_) => tracing::warn!(grace = ?READ_GRACE, "pump: p2p read stalled; re-kick"),
        }
    }
}
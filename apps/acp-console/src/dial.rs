//! 拨号 + 握手帧往返（需求 A）：facade 向期望 PeerId 开 /dsh-acp/1 流。
//! 传输层互认对端身份，实际 PeerId 与期望不符即拨号失败（PeerMismatch 显式上抛）；
//! 握手帧经 acp-common 编解码：conn=随机 uuid、token 可选透传、reattach 可选。

use std::io;
use std::time::Duration;

use acp_common::consts::{HANDSHAKE_VERSION, PROTOCOL_ID};
use acp_common::{frames, parse_server_hello, ClientHello, LineReassembler, ServerHello};
use p2p::{BoxedStream, Node, PeerId, ProtocolId};
use p2p_protocol::{read_frame, write_frame};
use tokio::io::AsyncWriteExt;
use uuid::Uuid;

/// 握手往返护栏：loopback 毫秒级，广域经中继也在数秒内。
pub const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Debug, thiserror::Error)]
pub enum DialError {
    #[error("dial: {0}")]
    Dial(String),
    #[error("handshake timeout after {0:?}")]
    Timeout(Duration),
    /// agent 侧可观察拒绝：denied 帧携带 acp-common 错误词法码。
    #[error("agent denied: {0}")]
    Denied(String),
    #[error("handshake malformed: {0}")]
    Malformed(String),
}

/// base58 → PeerId（与 CLI/facade 同规则）；解析失败为结构化错误。
pub fn parse_peer_id(s: &str) -> Result<PeerId, String> {
    let bytes = bs58::decode(s)
        .into_vec()
        .map_err(|e| format!("bad peer id base58: {e}"))?;
    let arr: [u8; 32] = bytes
        .try_into()
        .map_err(|_| format!("bad peer id length: {s}"))?;
    Ok(PeerId::from_bytes(arr))
}

/// 握手往返产物：conn 为本端连接 uuid，ticket 为桥在 ready 帧签发的续连票据
/// （apps/acp-agent/README.md 续连票据；旧桥无票据面则为 None）。
#[derive(Debug, Clone)]
pub struct HandshakeOutcome {
    pub conn: Uuid,
    pub ticket: Option<String>,
}

/// 开流并完成握手往返。返回握手产物与已握手的裸字节流
/// （协议 ID 首帧之后即纯字节面，ACP ndjson 由两端直读直写）。
pub async fn dial_and_handshake(
    node: &Node,
    expected: PeerId,
    agent_token: Option<String>,
    reattach: Option<Uuid>,
) -> Result<(PeerId, HandshakeOutcome, BoxedStream), DialError> {
    let protocol = ProtocolId::new(PROTOCOL_ID).map_err(|e| DialError::Dial(e.to_string()))?;
    let stream = node
        .new_stream(expected, protocol)
        .await
        .map_err(|e| DialError::Dial(e.to_string()))?;
    let (outcome, stream) = exchange_hello(stream, agent_token, reattach).await?;
    Ok((expected, outcome, stream))
}

async fn exchange_hello(
    mut stream: BoxedStream,
    agent_token: Option<String>,
    reattach: Option<Uuid>,
) -> Result<(HandshakeOutcome, BoxedStream), DialError> {
    let conn = Uuid::new_v4();
    let hello = ClientHello {
        v: HANDSHAKE_VERSION,
        conn,
        token: agent_token,
        reattach,
    };
    let line = hello
        .to_line()
        .map_err(|e| DialError::Malformed(e.to_string()))?;
    // wire 帧面（设计 §4.2-1）：握手行同样按 varint 长度前缀切帧，
    // 与 agent read_wire_line 同帧；裸写字节会被对端当作帧长解析（历史缺陷）。
    // frames() 自带行尾换行帧，无需手工 push('\n')。
    for frame in frames(line.as_bytes()) {
        write_frame(&mut stream, frame)
            .await
            .map_err(|e| DialError::Dial(format!("hello write: {e}")))?;
    }
    stream
        .flush()
        .await
        .map_err(|e| DialError::Dial(format!("hello flush: {e}")))?;
    read_hello(stream, conn).await
}

async fn read_hello(
    mut stream: BoxedStream,
    conn: Uuid,
) -> Result<(HandshakeOutcome, BoxedStream), DialError> {
    // 帧内字节精确读：不经 BufReader（缓冲残字会随 into_inner 丢失）。
    let mut reassembler = LineReassembler::new();
    let reply = loop {
        if let Some(line) = reassembler.take_line() {
            break String::from_utf8_lossy(&line).into_owned();
        }
        let frame = match tokio::time::timeout(HANDSHAKE_TIMEOUT, read_frame(&mut stream)).await {
            Err(_) => return Err(DialError::Timeout(HANDSHAKE_TIMEOUT)),
            Ok(Err(e)) if e.kind() == io::ErrorKind::UnexpectedEof => {
                return Err(DialError::Dial(
                    "connection closed before server hello".into(),
                ));
            }
            Ok(Err(e)) => return Err(DialError::Dial(format!("hello read: {e}"))),
            Ok(Ok(frame)) => frame,
        };
        reassembler
            .push_frame(&frame)
            .map_err(|e| DialError::Malformed(e.to_string()))?;
    };
    match parse_server_hello(reply.trim()) {
        Ok(ServerHello::Ready { ready }) => Ok((
            HandshakeOutcome {
                conn,
                ticket: ready.ticket,
            },
            stream,
        )),
        Ok(ServerHello::Denied { denied }) => Err(DialError::Denied(denied)),
        Err(code) => Err(DialError::Malformed(code.to_string())),
    }
}

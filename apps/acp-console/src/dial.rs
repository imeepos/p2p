//! 拨号 + 握手帧往返（需求 A）：facade 向期望 PeerId 开 /dsh-acp/1 流。
//! 传输层互认对端身份，实际 PeerId 与期望不符即拨号失败（PeerMismatch 显式上抛）；
//! 握手帧经 acp-common 编解码：conn=随机 uuid、token 可选透传、reattach 可选。

use std::time::Duration;

use acp_common::consts::{HANDSHAKE_VERSION, PROTOCOL_ID};
use acp_common::{parse_server_hello, ClientHello, ServerHello};
use p2p::{BoxedStream, Node, PeerId, ProtocolId};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
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

/// 开流并完成握手往返。返回本次连接的 conn uuid 与已握手的裸字节流
/// （协议 ID 首帧之后即纯字节面，ACP ndjson 由两端直读直写）。
pub async fn dial_and_handshake(
    node: &Node,
    expected: PeerId,
    agent_token: Option<String>,
    reattach: Option<Uuid>,
) -> Result<(PeerId, Uuid, BoxedStream), DialError> {
    let protocol = ProtocolId::new(PROTOCOL_ID).map_err(|e| DialError::Dial(e.to_string()))?;
    let stream = node
        .new_stream(expected, protocol)
        .await
        .map_err(|e| DialError::Dial(e.to_string()))?;
    let (conn, stream) = exchange_hello(stream, agent_token, reattach).await?;
    Ok((expected, conn, stream))
}

async fn exchange_hello(
    mut stream: BoxedStream,
    agent_token: Option<String>,
    reattach: Option<Uuid>,
) -> Result<(Uuid, BoxedStream), DialError> {
    let conn = Uuid::new_v4();
    let hello = ClientHello {
        v: HANDSHAKE_VERSION,
        conn,
        token: agent_token,
        reattach,
    };
    let mut line = hello
        .to_line()
        .map_err(|e| DialError::Malformed(e.to_string()))?;
    line.push('\n');
    stream
        .write_all(line.as_bytes())
        .await
        .map_err(|e| DialError::Dial(format!("hello write: {e}")))?;
    stream
        .flush()
        .await
        .map_err(|e| DialError::Dial(format!("hello flush: {e}")))?;
    read_hello(BufReader::new(stream), conn).await
}

async fn read_hello(
    mut reader: BufReader<BoxedStream>,
    conn: Uuid,
) -> Result<(Uuid, BoxedStream), DialError> {
    let mut reply = String::new();
    let n = tokio::time::timeout(HANDSHAKE_TIMEOUT, reader.read_line(&mut reply))
        .await
        .map_err(|_| DialError::Timeout(HANDSHAKE_TIMEOUT))?
        .map_err(|e| DialError::Dial(format!("hello read: {e}")))?;
    if n == 0 {
        return Err(DialError::Dial(
            "connection closed before server hello".into(),
        ));
    }
    let stream = reader.into_inner();
    match parse_server_hello(reply.trim()) {
        Ok(ServerHello::Ready { .. }) => Ok((conn, stream)),
        Ok(ServerHello::Denied { denied }) => Err(DialError::Denied(denied)),
        Err(code) => Err(DialError::Malformed(code.to_string())),
    }
}

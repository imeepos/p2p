//! agent 模拟端点：acp-common 握手应答 + echo 泵，可配拒绝/签票/握手后半关闭。
//! protocol 可切 /a2a/1（?proto=a2a 通道测试），行为面完全同 echo。

use std::sync::Mutex;

use acp_common::consts::PROTOCOL_ID;
use acp_common::policy::Scope;
use acp_common::{frames, parse_client_hello, ClientHello, LineReassembler, ServerHello};
use p2p::{BoxedStream, ProtocolHandler, ProtocolId};
use p2p_protocol::{read_frame, write_frame};
use tokio::io::AsyncWriteExt;

/// agent 模拟端点：握手应答 + 字节 echo；可配置拒绝码、签发票据与握手后半关闭探针。
/// protocol 可切 /a2a/1（?proto=a2a 通道测试），行为面完全同 echo。
pub struct AgentMock {
    deny: Option<String>,
    drop_after_ready: bool,
    half_close_after_ready: bool,
    issue_ticket: Option<String>,
    protocol: &'static str,
    received: Mutex<Option<ClientHello>>,
    /// 全部握手历史（share 直拨等多次连接断言用）。
    history: Mutex<Vec<ClientHello>>,
}

impl AgentMock {
    pub fn echo() -> Self {
        Self {
            deny: None,
            drop_after_ready: false,
            half_close_after_ready: false,
            issue_ticket: None,
            protocol: PROTOCOL_ID,
            received: Mutex::new(None),
            history: Mutex::new(Vec::new()),
        }
    }

    /// echo 行为挂 /a2a/1 协议 ID（卡片事件通道对端桩）。
    pub fn echo_a2a() -> Self {
        Self {
            protocol: a2a::PROTOCOL_ID,
            ..Self::echo()
        }
    }

    pub fn denying(code: &str) -> Self {
        Self {
            deny: Some(code.to_string()),
            ..Self::echo()
        }
    }

    /// 握手后就地流级 shutdown（探针：锁定底座半关闭 FIN→EOF 语义）。
    pub fn half_closing() -> Self {
        Self {
            half_close_after_ready: true,
            ..Self::echo()
        }
    }

    /// echo + ready 帧签发续连票据（桥约定：票据进 ready，客户端携回重连）。
    pub fn echo_with_ticket(ticket: &str) -> Self {
        Self {
            issue_ticket: Some(ticket.to_string()),
            ..Self::echo()
        }
    }

    /// 收到的 ClientHello（None = 尚未握手；多次连接取最近一次）。
    pub fn hello(&self) -> Option<ClientHello> {
        self.received.lock().unwrap().clone()
    }

    /// 按到达序的全部握手记录（幂等/重复导入断言用）。
    pub fn hellos(&self) -> Vec<ClientHello> {
        self.history.lock().unwrap().clone()
    }
}

#[async_trait::async_trait]
impl ProtocolHandler for AgentMock {
    fn protocol(&self) -> ProtocolId {
        ProtocolId::new(self.protocol).unwrap()
    }

    async fn handle(&self, mut stream: BoxedStream) -> std::io::Result<()> {
        // wire 帧面（设计 §4.2-1）：握手与 echo 都走 varint 帧，与生产 agent 同帧。
        let hello = read_framed_line(&mut stream).await?;
        let hello = parse_client_hello(hello.trim())
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))?;
        *self.received.lock().unwrap() = Some(hello.clone());
        self.history.lock().unwrap().push(hello);
        let reply = match (&self.deny, &self.issue_ticket) {
            (Some(code), _) => ServerHello::Denied {
                denied: code.clone(),
            },
            (None, Some(ticket)) => {
                ServerHello::ready_with_ticket(Scope::Sandbox, "mock-agent", ticket)
            }
            (None, None) => ServerHello::ready(Scope::Sandbox, "mock-agent"),
        };
        let reply_line = reply
            .to_line()
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))?;
        let mut stream = write_framed_line(stream, reply_line.as_bytes()).await?;
        if self.half_close_after_ready {
            // 探针模式：就地流级 shutdown，验证 FIN→EOF 对端可见（见
            // transport_semantics.rs 与治理文档）。
            stream.shutdown().await?;
            return Ok(());
        }
        if self.drop_after_ready {
            // 静默返回（流 drop 不发 FIN 也不 reset）：真实断流场景用连接级
            // shutdown 模拟（见 transport_semantics 探针与 E-4）。
            return Ok(());
        }
        echo_loop(stream).await
    }
}

/// 帧化 echo 泵：帧读 + 行重组，整行（含行尾换行）原样经 frames() 回写，EOF 即结束。
async fn echo_loop(mut stream: BoxedStream) -> std::io::Result<()> {
    let mut reassembler = LineReassembler::new();
    loop {
        let frame = match read_frame(&mut stream).await {
            Ok(frame) => frame,
            Err(err) if err.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(()),
            Err(err) => return Err(err),
        };
        if let Err(e) = reassembler.push_frame(&frame) {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                e.to_string(),
            ));
        }
        while let Some(mut line) = reassembler.take_line() {
            while line.last() == Some(&10) {
                line.pop();
            }
            stream = write_framed_line(stream, &line).await?;
        }
    }
}

/// 读一条帧化 ndjson 行（帧内字节精确读，无缓冲残字问题）。
async fn read_framed_line(stream: &mut BoxedStream) -> std::io::Result<String> {
    let mut reassembler = LineReassembler::new();
    loop {
        if let Some(line) = reassembler.take_line() {
            return Ok(String::from_utf8_lossy(&line).into_owned());
        }
        let frame = read_frame(stream).await?;
        reassembler
            .push_frame(&frame)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))?;
    }
}

/// 写一条帧化 ndjson 行（frames() 自带行尾换行帧）。
async fn write_framed_line(mut stream: BoxedStream, line: &[u8]) -> std::io::Result<BoxedStream> {
    for frame in frames(line) {
        write_frame(&mut stream, frame).await?;
    }
    stream.flush().await?;
    Ok(stream)
}

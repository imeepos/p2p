//! 回环夹具：agent 模拟节点（acp-common 握手应答 + echo 泵，可配拒绝/握手后即断）
//! 与 console 侧组件栈（真实双 Node loopback + WS 服务，端口 0）。
//! 只放装置与有界等待；断言留在各测试。
//!
//! 共享夹具按目标独立编译：单测试目标用不到的装置在此统一豁免 dead_code。
#![allow(dead_code)]

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use acp_common::consts::PROTOCOL_ID;
use acp_common::policy::Scope;
use acp_common::{frames, parse_client_hello, ClientHello, LineReassembler, ServerHello};
use p2p::{BoxedStream, Node, PeerId, ProtocolHandler, ProtocolId};
use p2p_protocol::{read_frame, write_frame};
use tokio::io::AsyncWriteExt;

use acp_console::discovery::DiscoveryHub;
use acp_console::state::StatusHub;
use acp_console::ticket::TicketStore;
use acp_console::ws::{WsDeps, WsServer};

/// 单步等待上限：loopback 毫秒级，10s 是宽松护栏。
pub const STEP: Duration = Duration::from_secs(10);
/// 测试用续连窗口：短窗让 offline 迁移在测试内可见。
pub const TEST_WINDOW: Duration = Duration::from_millis(400);

/// agent 模拟端点：握手应答 + 字节 echo；可配置拒绝码、签发票据与握手后半关闭探针。
pub struct AgentMock {
    deny: Option<String>,
    drop_after_ready: bool,
    half_close_after_ready: bool,
    issue_ticket: Option<String>,
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
            received: Mutex::new(None),
            history: Mutex::new(Vec::new()),
        }
    }

    pub fn denying(code: &str) -> Self {
        Self {
            deny: Some(code.to_string()),
            drop_after_ready: false,
            half_close_after_ready: false,
            issue_ticket: None,
            received: Mutex::new(None),
            history: Mutex::new(Vec::new()),
        }
    }

    /// 握手后就地流级 shutdown（探针：锁定底座半关闭 FIN→EOF 语义）。
    pub fn half_closing() -> Self {
        Self {
            deny: None,
            drop_after_ready: false,
            half_close_after_ready: true,
            issue_ticket: None,
            received: Mutex::new(None),
            history: Mutex::new(Vec::new()),
        }
    }

    /// echo + ready 帧签发续连票据（桥约定：票据进 ready，客户端携回重连）。
    pub fn echo_with_ticket(ticket: &str) -> Self {
        Self {
            deny: None,
            drop_after_ready: false,
            half_close_after_ready: false,
            issue_ticket: Some(ticket.to_string()),
            received: Mutex::new(None),
            history: Mutex::new(Vec::new()),
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
        ProtocolId::new(PROTOCOL_ID).unwrap()
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
async fn write_framed_line(
    mut stream: BoxedStream,
    line: &[u8],
) -> std::io::Result<BoxedStream> {
    for frame in frames(line) {
        write_frame(&mut stream, frame).await?;
    }
    stream.flush().await?;
    Ok(stream)
}

/// console 侧组件栈 + 两端节点。
pub struct Rig {
    pub agent: Node,
    pub agent_peer: PeerId,
    pub console: Arc<Node>,
    pub hub: Arc<StatusHub>,
    pub disc: Arc<DiscoveryHub>,
    pub tickets: Arc<TicketStore>,
    pub mock: Arc<AgentMock>,
    pub ws_addr: SocketAddr,
    pub token: String,
    pub data_dir: PathBuf,
}

/// 起一套回环装置：agent 节点（挂 mock）+ console 节点 + WS 服务（随机端口）。
pub async fn rig(tag: &str, mock: AgentMock) -> Rig {
    let _ = p2p_log::init(Default::default());
    let base = std::env::temp_dir().join(format!("acp-console-rig-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    std::fs::create_dir_all(&base).unwrap();

    let mock = Arc::new(mock);
    let agent = Node::builder()
        .mdns(false)
        .data_dir(base.join("agent-id"))
        .build()
        .await
        .unwrap();
    agent.handle_protocol(mock.clone() as Arc<dyn ProtocolHandler>);
    let agent_peer = agent.local_peer_id();

    let console = Arc::new(
        Node::builder()
            .mdns(false)
            .data_dir(base.join("console-id"))
            .build()
            .await
            .unwrap(),
    );
    for addr in agent.listen_addrs() {
        if addr.contains("/t") {
            console.add_peer_address(agent_peer, &addr).unwrap();
        }
    }
    console.connect(agent_peer).await.unwrap();

    let hub = Arc::new(StatusHub::new());
    let disc = Arc::new(DiscoveryHub::default());
    let tickets = Arc::new(TicketStore::new(&base));
    let token = acp_console::token::new_token();
    let ws = WsServer::start(
        0,
        token.clone(),
        WsDeps {
            node: console.clone(),
            hub: hub.clone(),
            tickets: tickets.clone(),
            window: TEST_WINDOW,
        },
    )
    .await
    .unwrap();

    Rig {
        agent,
        agent_peer,
        console,
        hub,
        disc,
        tickets,
        mock,
        ws_addr: ws.addr,
        token,
        data_dir: base,
    }
}

/// 客户端 WS 流类型：connect_async 经 MaybeTls 包裹。
pub type WsStream =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

/// 带 token 与目标 peer 的 WS 连接（成功路径）。
pub async fn ws_connect(rig: &Rig) -> WsStream {
    ws_try_connect_with(rig, &format!("token={}&peer={}", rig.token, rig.agent_peer))
        .await
        .expect("ws connect")
}

/// 自定义 query 串（可注入错 token / 缺参等坏形），保留原始错误供断言。
pub async fn ws_try_connect_with(
    rig: &Rig,
    query: &str,
) -> Result<
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>,
    tokio_tungstenite::tungstenite::Error,
> {
    let url = format!("ws://{}/?{query}", rig.ws_addr);
    tokio_tungstenite::connect_async(url)
        .await
        .map(|(ws, _)| ws)
}

/// 关停两端节点并清理夹具目录。
pub fn teardown(rig: Rig) {
    rig.agent.shutdown();
    rig.console.shutdown();
    let _ = std::fs::remove_dir_all(&rig.data_dir);
}

/// status 端点依赖（与 main.rs 同装配：hub + discovery + 票据 + 窗口 + 直拨面）。
pub fn status_deps(rig: &Rig) -> acp_console::status::StatusDeps {
    acp_console::status::StatusDeps {
        hub: rig.hub.clone(),
        discovery: rig.disc.clone(),
        tickets: rig.tickets.clone(),
        window: TEST_WINDOW,
        node: rig.console.clone(),
        ws_addr: rig.ws_addr,
        ws_token: rig.token.clone(),
    }
}

mod util;
// 按目标独立编译：不触 HTTP 面的测试目标会报未用，沿本文件 dead_code 豁免先例。
#[allow(unused_imports)]
pub use util::*;

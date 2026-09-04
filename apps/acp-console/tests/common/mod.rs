//! 回环夹具：agent 模拟节点（acp-common 握手应答 + echo 泵，可配拒绝/握手后即断）
//! 与 console 侧组件栈（真实双 Node loopback + WS 服务，端口 0）。
//! 只放装置与有界等待；断言留在各测试。

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use acp_common::consts::PROTOCOL_ID;
use acp_common::policy::Scope;
use acp_common::{parse_client_hello, ClientHello, ServerHello};
use p2p::{BoxedStream, Node, PeerId, ProtocolHandler, ProtocolId};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};

use acp_console::discovery::DiscoveryHub;
use acp_console::state::StatusHub;
use acp_console::ticket::TicketStore;
use acp_console::ws::{WsDeps, WsServer};

/// 单步等待上限：loopback 毫秒级，10s 是宽松护栏。
pub const STEP: Duration = Duration::from_secs(10);
/// 测试用续连窗口：短窗让 offline 迁移在测试内可见。
pub const TEST_WINDOW: Duration = Duration::from_millis(400);

/// agent 模拟端点：握手应答 + 字节 echo；可配置拒绝码与握手后即断。
pub struct AgentMock {
    deny: Option<String>,
    drop_after_ready: bool,
    received: Mutex<Option<ClientHello>>,
}

impl AgentMock {
    pub fn echo() -> Self {
        Self {
            deny: None,
            drop_after_ready: false,
            received: Mutex::new(None),
        }
    }

    pub fn denying(code: &str) -> Self {
        Self {
            deny: Some(code.to_string()),
            drop_after_ready: false,
            received: Mutex::new(None),
        }
    }

    /// 收到的 ClientHello（None = 尚未握手）。
    pub fn hello(&self) -> Option<ClientHello> {
        self.received.lock().unwrap().clone()
    }
}

#[async_trait::async_trait]
impl ProtocolHandler for AgentMock {
    fn protocol(&self) -> ProtocolId {
        ProtocolId::new(PROTOCOL_ID).unwrap()
    }

    async fn handle(&self, stream: BoxedStream) -> std::io::Result<()> {
        let mut reader = BufReader::new(stream);
        let mut line = String::new();
        reader.read_line(&mut line).await?;
        let hello = parse_client_hello(line.trim())
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))?;
        *self.received.lock().unwrap() = Some(hello);
        let reply = match &self.deny {
            Some(code) => ServerHello::Denied {
                denied: code.clone(),
            },
            None => ServerHello::ready(Scope::Sandbox, "mock-agent"),
        };
        let mut out = reply
            .to_line()
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))?;
        out.push('\n');
        reader.get_mut().write_all(out.as_bytes()).await?;
        reader.get_mut().flush().await?;
        if self.drop_after_ready {
            // 注意：本底座经 Compat 的流级 shutdown 为 no-op（futures 默认实现），
            // 半关闭无原语；断流场景在测试中以 agent 节点连接级 shutdown 模拟。
            return Ok(());
        }
        echo_loop(reader.into_inner()).await
    }
}

/// 有界 echo 泵：64 KiB 块读到多少回多少，EOF 即结束。
async fn echo_loop(mut stream: BoxedStream) -> std::io::Result<()> {
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        let n = stream.read(&mut buf).await?;
        if n == 0 {
            return Ok(());
        }
        stream.write_all(&buf[..n]).await?;
        stream.flush().await?;
    }
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

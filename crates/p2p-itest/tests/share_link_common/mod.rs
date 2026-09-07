//! SHARE 波 E2E 夹具·节点与直拨（docs/design/acp-share-design.md §4/§5/§7）：
//! owner = 进程内真 acp-agent 桥（SessionDeps/AcpHandler/ShareService/admin HTTP
//! 全真实装配），guest = acp-console 库面解析链接后按 dial.rs 同机制直拨握手；
//! 两节点真 QUIC loopback。子进程用 POSIX sh 回声桩（crates 侧不依赖
//! acp-echo-stub bin）：stdout 首行 = 子进程 cwd（监狱落位观测），其余逐行回声。
//! 断言全部在 share_link_wave.rs；admin 客户端与断言工具见 support.rs。

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

pub mod support;

use acp_agent::share::admin::{AdminDeps, AdminServer, AdminToken};
use acp_agent::share::LinkContext;
use acp_agent::{AcpHandler, AgentConfig, CaptureAudit, SessionDeps};
use acp_common::consts::{HANDSHAKE_VERSION, PROTOCOL_ID};
use acp_common::{ClientHello, PolicyTable, ServerHello};
use p2p::{BoxedStream, Node, PeerId, ProtocolId};
use uuid::Uuid;

pub use support::{
    admin_call, create_share, expect_denied, expect_ready, line_within, read_line, sandbox_jail,
    send_line, skip_signal, wait_redeem_denied, wait_share_redeemed,
};

/// 单步等待上限：本地 loopback 毫秒级，15s 为宽松护栏。
pub const STEP: Duration = Duration::from_secs(15);

pub fn tmp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("share-e2e-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("tmp dir");
    dir
}

/// 回声桩（语义同源 acp-echo-stub 的 print-cwd + 行回声 + acp-stub-exit 哨兵）。
pub fn stub_command() -> Vec<String> {
    let script = "pwd
while IFS= read -r l; do
  case \"$l\" in *acp-stub-exit*) exit 0;; esac
  printf '%s\\n' \"$l\"
done";
    vec!["/bin/sh".to_owned(), "-c".to_owned(), script.to_owned()]
}

/// owner 台架：真桥节点 + 真台账 + 真 admin HTTP（Bearer 鉴权，127.0.0.1）。
pub struct OwnerRig {
    pub node: Node,
    pub peer: PeerId,
    pub deps: Arc<SessionDeps>,
    pub audit: Arc<CaptureAudit>,
    pub admin_addr: SocketAddr,
    pub admin_token: String,
    root: PathBuf,
}

impl Drop for OwnerRig {
    fn drop(&mut self) {
        self.node.shutdown();
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

pub async fn owner_rig(tag: &str, tweak: impl FnOnce(&mut AgentConfig)) -> OwnerRig {
    let root = tmp_dir(tag);
    let data_dir = root.join("owner");
    std::fs::create_dir_all(&data_dir).expect("owner data dir");
    let mut cfg = AgentConfig {
        data_dir: data_dir.to_string_lossy().into_owned(),
        command: stub_command(),
        grace_secs: 1,
        reattach_window_secs: 1,
        permission_timeout_secs: 2,
        ..AgentConfig::default()
    };
    tweak(&mut cfg);
    let policy_path = cfg.policy_path();
    std::fs::create_dir_all(policy_path.parent().expect("policy parent")).expect("mkdir policy");
    PolicyTable::new()
        .save(&policy_path)
        .expect("save empty policy");
    let node = Node::builder()
        .mdns(false)
        .data_dir(data_dir.join("identity"))
        .build()
        .await
        .expect("owner node");
    let audit = Arc::new(CaptureAudit::new());
    let deps = SessionDeps::assemble(cfg.clone(), audit.clone()).expect("assemble");
    node.handle_protocol(Arc::new(AcpHandler::new(deps.clone()).expect("handler")));
    let peer = node.local_peer_id();
    let quic_addrs: Vec<String> = node
        .listen_addrs()
        .into_iter()
        .filter(|a| a.contains("/u"))
        .collect();
    assert!(!quic_addrs.is_empty(), "owner node must listen on QUIC");
    let token = AdminToken::issue(cfg.paths().admin_token()).expect("admin token");
    let server = AdminServer::start(
        cfg.admin_port,
        token.value.clone(),
        AdminDeps {
            service: deps.shares.clone(),
            link: LinkContext {
                peer: peer.to_string(),
                addrs: quic_addrs,
            },
            // share_link_wave 不涉多工作区定向，空清单即可（GET /workspaces 返回空）
            workspaces: Arc::new(
                acp_agent::workspaces::WorkspaceStore::open(&[], None, cfg.paths().workspaces())
                    .expect("ws store"),
            ),
        },
    )
    .await
    .expect("admin server");
    OwnerRig {
        node,
        peer,
        deps,
        audit,
        admin_addr: server.addr,
        admin_token: token.value,
        root,
    }
}

/// guest 节点（独立身份，Drop 关停并清目录）。
pub struct Guest {
    pub node: Node,
    pub peer: PeerId,
    root: PathBuf,
}

impl Drop for Guest {
    fn drop(&mut self) {
        self.node.shutdown();
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

pub async fn guest_node(tag: &str) -> Guest {
    let root = tmp_dir(tag);
    let node = Node::builder()
        .mdns(false)
        .data_dir(root.join("identity"))
        .build()
        .await
        .expect("guest node");
    let peer = node.local_peer_id();
    Guest { node, peer, root }
}

/// 链接直拨产物：Ready 帧原样上抛（scope 断言用）或 denied 码。
pub enum DialVerdict {
    Ready {
        hello: ServerHello,
        stream: BoxedStream,
    },
    Denied {
        code: String,
    },
}

/// guest 侧直拨（acp-console share/link.rs 真解析 + dial.rs 同机制握手）：
/// 解析链接 → 登记 QUIC 候选 → connect → 开流 → ClientHello.token = 链接 token。
pub async fn dial_link(
    guest: &Guest,
    raw_link: &str,
) -> Result<(acp_console::share::ShareLink, DialVerdict), String> {
    let link =
        acp_console::share::parse_share_link(raw_link).map_err(|e| format!("parse link: {e}"))?;
    for addr in &link.addrs {
        if addr.contains("/u") {
            guest
                .node
                .add_peer_address(link.peer_id, addr)
                .map_err(|e| format!("seed addr: {e}"))?;
        }
    }
    guest
        .node
        .connect(link.peer_id)
        .await
        .map_err(|e| format!("dial: {e}"))?;
    let protocol = ProtocolId::new(PROTOCOL_ID).map_err(|e| format!("protocol: {e}"))?;
    let mut stream = guest
        .node
        .new_stream(link.peer_id, protocol)
        .await
        .map_err(|e| format!("open stream: {e}"))?;
    let hello = ClientHello {
        v: HANDSHAKE_VERSION,
        conn: Uuid::new_v4(),
        token: Some(link.token.clone()),
        reattach: None,
    };
    // 握手走冻结线契约（varint 帧，桥 pump.rs read/write_wire_line 同款）。
    // 注意：acp-console/src/dial.rs 此处为裸 ndjson 读写，与真桥不符——bug 已登记回执，
    // 本夹具不复制该缺陷；链接解析仍用 acp-console 真实现 parse_share_link。
    let hello_line = hello.to_line().map_err(|e| format!("hello line: {e}"))?;
    send_line(&mut stream, &hello_line).await;
    let reply = match tokio::time::timeout(STEP, read_line(&mut stream)).await {
        Ok(Some(line)) => line,
        Ok(None) => {
            return Ok((
                link,
                DialVerdict::Denied {
                    code: "connection-closed".to_owned(),
                },
            ))
        }
        Err(_) => return Err("handshake timeout".to_owned()),
    };
    match acp_common::parse_server_hello(reply.trim_end()) {
        Ok(hello @ ServerHello::Ready { .. }) => Ok((link, DialVerdict::Ready { hello, stream })),
        Ok(ServerHello::Denied { denied }) => Ok((link, DialVerdict::Denied { code: denied })),
        Err(err) => Err(format!("malformed hello: {err}")),
    }
}

/// 撤销后再连：上一会话槽位释放是异步的，conn-cap 视为忙碌有界重试。
pub async fn dial_link_fresh_slot(
    guest: &Guest,
    raw_link: &str,
) -> (acp_console::share::ShareLink, DialVerdict) {
    let deadline = tokio::time::Instant::now() + STEP;
    loop {
        let (link, verdict) = dial_link(guest, raw_link).await.expect("dial by link");
        let busy = matches!(&verdict, DialVerdict::Denied { code } if code.contains("conn-cap"));
        if !busy || tokio::time::Instant::now() >= deadline {
            return (link, verdict);
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

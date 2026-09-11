# PROGRESS — TA：tunnel 泛化契约冻结 + 访侧反代核心签名桩

分支 feat/wta-tunnel-contract ｜ worktree .worktrees/wta-contract ｜ 基于 origin/main @ a9ea63e1
类型 architecture ｜ 状态：DONE（全部门禁绿，已推 origin，禁自合并）

## 分段进度

1. [x] 读 AGENTS.md / 任务书 / proxy.rs+head.rs+pump.rs（272+270+94 行）/ p2p-tunnel lib.rs+audit.rs
   / gui-contract §19 / specs/tunnel.md §3.3+§8 / registry.toml（impl="implemented" 已核对）
2. [x] 建树：`git worktree add .worktrees/wta-contract -b feat/wta-tunnel-contract origin/main`
3. [x] 签名桩 b2952fbc：local_proxy/{mod,head,pump}.rs + lib.rs 挂模块 re-export；
   桩体一律 `todo!("W-TB")`；GUI 依赖零引入（见「提炼决策」）
4. [x] p2p-tunnel 门禁：`cargo check -p p2p-tunnel` EXIT=0；
   `cargo clippy -p p2p-tunnel --all-targets -- -D warnings` EXIT=0；
   `cargo fmt --check` EXIT=0（首跑 lib.rs use 排序 diff → `cargo fmt` 后复验过）
5. [x] gui-contract.md：§19.1 加 tunnel_open 表行；§19.3 约束 8 补 `p2pctl tunnel connect`
   对等 exempt；新增约束 9（编号顺延现文确认：CLI 对等条款 = §19.3 有序列表第 8 项，
   specs/tunnel.md §5.2 引用锚点即此）
6. [x] specs/tunnel.md §8：实现状态改已实现口径（与 registry.toml 一致）+ 漂移登记
   （反代核心现状 apps/gui 承载 → 下沉 crates/p2p-tunnel local_proxy，迁移卡 TB）；
   §1-§7 一字未动；冻结接口表 LocalProxy 行归属不变
7. [x] `cargo check --workspace` EXIT=0（4m22s）
8. [x] push origin feat/wta-tunnel-contract（禁自合并，合并归协调者）

## 提交拆分（桩与登记类分离）

- b2952fbc feat(tunnel): local_proxy 访侧反代核心签名桩（W-TA 契约先行）
- e53dec59 docs(design): gui-contract §19 冻结 tunnel_open 通用命令契约（独立小提交）
- df259a49 docs(protocol): tunnel.md §8 实现状态口径与漂移登记（独立小提交）
- 334184e2 docs(orchestrator): PROGRESS（本文件）
- 5770c15a chore(skill): 经验喂回（append-only，走分支流程）
- （本修订）docs(orchestrator): 签名表升级为逐字源码形态

## 提炼决策（TB/TC/TD 必读）

1. **PumpAudit trait 缝**：GUI pump.rs 的 forward_exact/drain_reply 消费 `&ConnAudit`
   （GUI 类型，字节计数 add_in/add_out）。crate 侧提炼为
   `pub trait PumpAudit: Send + Sync { fn add_in(&self, n: u64); fn add_out(&self, n: u64); }`，
   pump 函数签名取 `&dyn PumpAudit`。TB 迁移时由 GUI ConnAudit 实现该 trait，
   或以 crate 内会话审计结构承载后 GUI 适配。
2. **审计快照换型**：ProxyCtx.audit_snapshot 返回 `Vec<TunnelAuditRecord>`（crate 内
   §5.2 八字段闭集，snake_case），替代 GUI 的 `Vec<TunnelSessionAudit>`（camelCase）。
   camelCase 映射留在 GUI 装配层（tunnel_status emit 前），语义与 §19.3-5 一致。
   audit_snapshot 保留 async 形状（GUI AuditLog 为异步账，TB 平移成本低）。
3. **ProxyCtx/LocalProxy 私有字段 + `#[allow(dead_code)]`**：桩期字段无消费点，
   deny(warnings) 下需显式放行；TB 填实现后随手移除 allow。mod.rs 附带 crate 内
   构造器与 conns 计数私有辅助（同样 allow 标注）。
4. **零新依赖**：local_proxy 只用 std + tokio（workspace full features）+ async-trait +
   crate 既有导出（TunnelIo/TunnelError/TunnelAuditRecord），Cargo.toml 未动；
   bytes/http 未引入（纯函数面全部 `Vec<u8>`/`&[u8]`，与现实现同口径）。
5. **未提炼面（实现细节，TB 自带）**：serve_conn/forward_conn/upgrade_path/plain_path/
   reject/failure_code/new_uid（getrandom 依赖在 GUI）/fallback_audit 均为私有实现，
   不属冻结面；head.rs 的 find_head_end、rewrite_loopback_url 同理（私有纯函数）。
6. **契约文本**：tunnel_open 与 tunnel_open_dsh 并存不废弃（§19.3-9 冻结分层语义）；
   token 空串承载「无 token」，openUrl = local_addr 本身；connect 对等 exempt 援引
   serve 条款措辞（进程活语义，无 GUI 事件面可对等）。

## 桩签名逐字清单（与 b2952fbc 源码逐字一致）

以下三块为 b2952fbc 提交的三个文件**完整逐字源码**（含全部 pub 项的属性、字段、
签名与桩体；fmt 后形态，`cargo fmt --check` EXIT=0 的即此文本）。lib.rs 的冻结
re-export 面单列在后。

### crates/p2p-tunnel/src/local_proxy/mod.rs（逐字）

```rust
//! 访侧本地回环反代核心（契约 §3.3 / gui-contract §19.3）：签名桩（W-TA 契约先行）。
//! 实现填埋 = W-TB（迁移自 apps/gui/src-tauri/src/tunnel/{proxy,head,pump}.rs，
//! GUI 切换同卡）。只绑 `127.0.0.1` 字面量；一条浏览器连接 ↔ 一条隧道流；
//! 每请求重写 Host 与同面 Origin/Referer；响应与 body 流式转发禁整包缓冲。
//! 停止 = abort serve 任务（listener 无部分状态；活动连接自然排空）。

pub mod head;
pub mod pump;

use std::net::SocketAddr;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

use crate::audit::{TunnelAudit, TunnelAuditRecord};
use crate::{TunnelError, TunnelIo};

/// 开隧道缝：产出 ack 后的裸字节流（生产 = [crate::TunnelClient] 包装；
/// 单测 = 自足握手的直连裸流）。独立 trait 使反代测试免于耦合 TunnelClient 内部。
///
/// 出处：apps/gui/src-tauri/src/tunnel/proxy.rs:24-27（现形状逐字提炼）；
/// 数据面语义 specs/tunnel.md §3.1-§3.3。
#[async_trait::async_trait]
pub trait TunnelOpener: Send + Sync {
    /// `uid` = 16 hex 会话 id（两侧日志同源，§5.2）；`target` = `127.0.0.1:<port>`
    /// 字面量；成功返回 ack 后的 [TunnelIo]，拒绝/故障返回 [TunnelError]
    /// （错误码闭集 §4）。
    async fn open(&self, uid: &str, target: &str) -> Result<TunnelIo, TunnelError>;
}

/// 反代共享上下文（GUI 装配的 `AuditLog`/`TunnelSessionAudit` 以 crate 内
/// [TunnelAudit]/[TunnelAuditRecord] 承载，camelCase 映射留在产品装配层）。
#[allow(dead_code)] // W-TB 填实现后随实现消费移除
pub struct ProxyCtx {
    target_port: u16,
    peer_id: String,
    opener: Arc<dyn TunnelOpener>,
    conns: AtomicU32,
    audit: TunnelAudit,
}

impl ProxyCtx {
    /// 活动连接数（tunnel_status 观测面）。
    /// 出处：apps/gui/src-tauri/src/tunnel/proxy.rs:39-41。
    pub fn active_conns(&self) -> u32 {
        todo!("W-TB")
    }

    /// 审计快照（tunnel_status 的 sessions 字段，§5.2 八字段闭集）。
    /// 出处：apps/gui/src-tauri/src/tunnel/proxy.rs:43-47（async 形状保留，
    /// 便于 TB 以现有异步审计账平移）。
    pub async fn audit_snapshot(&self) -> Vec<TunnelAuditRecord> {
        todo!("W-TB")
    }
}

/// 本地反代监听体。`bind` 只绑 127.0.0.1 字面量；`serve` 由调用方 spawn，
/// 停止 = abort 该任务（listener 无部分状态；活动连接自然排空）。
/// 出处：apps/gui/src-tauri/src/tunnel/proxy.rs:49-102；绑定面 §19.3-1 / §3.3。
#[allow(dead_code)] // W-TB 填实现后随实现消费移除
pub struct LocalProxy {
    listener: tokio::net::TcpListener,
    pub ctx: Arc<ProxyCtx>,
}

impl LocalProxy {
    /// 绑定 127.0.0.1:0（§19.3-1 / §3.3：禁 localhost/0.0.0.0/::1，port 0 由
    /// OS 分配）；`target_port` = 被访目标端口（target 字面量由此拼装）；
    /// `peer_id` = 被访节点 PeerId（审计字段）；`opener` = 隧道开启缝。
    /// 出处：apps/gui/src-tauri/src/tunnel/proxy.rs:57-74。
    pub async fn bind(
        target_port: u16,
        peer_id: String,
        opener: Arc<dyn TunnelOpener>,
    ) -> std::io::Result<Self> {
        let _ = (target_port, peer_id, opener);
        todo!("W-TB")
    }

    /// 反代监听地址（bind 成功后必有值）。
    /// 出处：apps/gui/src-tauri/src/tunnel/proxy.rs:76-78。
    pub fn local_addr(&self) -> SocketAddr {
        todo!("W-TB")
    }

    /// accept 循环；非回环来源直接拒（防绑定面意外暴露）。
    /// 出处：apps/gui/src-tauri/src/tunnel/proxy.rs:80-101。
    pub async fn serve(self) {
        todo!("W-TB")
    }
}

impl ProxyCtx {
    #[allow(dead_code)] // W-TB 构造器；随实现落地一并转正
    fn new(
        target_port: u16,
        peer_id: String,
        opener: Arc<dyn TunnelOpener>,
        audit: TunnelAudit,
    ) -> Self {
        Self {
            target_port,
            peer_id,
            opener,
            conns: AtomicU32::new(0),
            audit,
        }
    }

    #[allow(dead_code)] // W-TB 填实现后随实现消费移除
    fn conns_add(&self) {
        self.conns.fetch_add(1, Ordering::Relaxed);
    }

    #[allow(dead_code)] // W-TB 填实现后随实现消费移除
    fn conns_sub(&self) {
        self.conns.fetch_sub(1, Ordering::Relaxed);
    }

    #[allow(dead_code)] // W-TB 填实现后随实现消费移除
    fn audit(&self) -> &TunnelAudit {
        &self.audit
    }
}
```

### crates/p2p-tunnel/src/local_proxy/head.rs（逐字）

```rust
//! HTTP 报文头读写（契约 §6 兼容面 / §3.3 重写规则 / §19.3-2）：签名桩。
//! Host/Origin/Referer 重写 + hop-by-hop 取舍 + 请求头解析面；只切头不解释体，
//! 头后字节必须原样透传（含 leftover，禁缓冲丢失）。实现 = W-TB（迁移自
//! apps/gui/src-tauri/src/tunnel/head.rs，纯函数无外部依赖）。

/// 报文头上限：浏览器请求头与响应头都在数 KiB 量级，32 KiB 足够宽；超限显式
/// 报错防失控。出处：apps/gui/src-tauri/src/tunnel/head.rs:6-7。
pub const HEAD_MAX: usize = 32 * 1024;

/// 解析后的 HTTP 头（首行 + 头键值对，键保留原样大小写）。
/// 出处：apps/gui/src-tauri/src/tunnel/head.rs:9-14。
#[derive(Clone, Debug)]
pub struct Head {
    pub first_line: String,
    pub headers: Vec<(String, String)>,
}

impl Head {
    /// 解析原始头字节（不含头后 body）：首行非空 + 头行 `key: value`；
    /// 非 UTF-8 / 空首行 / 缺冒号为 Err。出处：head.rs:16-39。
    pub fn parse(bytes: &[u8]) -> Result<Self, String> {
        let _ = bytes;
        todo!("W-TB")
    }

    /// 按名取头（ASCII 大小写不敏感），多值取首个。出处：head.rs:41-46。
    pub fn header(&self, name: &str) -> Option<&str> {
        let _ = name;
        todo!("W-TB")
    }

    /// 覆写或追加头（保持其余头原样）。出处：head.rs:48-58。
    pub fn set_header(&mut self, name: &str, value: &str) {
        let _ = (name, value);
        todo!("W-TB")
    }

    /// 契约 §3.3：重写 Host 为目标 `127.0.0.1:<port>`；同面的 Origin/Referer
    /// 存在且 authority host 为回环字面量时重写为同一目标 authority（来源否则
    /// 停在反代端口上，被目标按 authority 校验拒绝写请求与 WS）；无该头不造头，
    /// 非回环/`null` 原样保留；重写目标仅限票据 target 的回环 authority，
    /// 不扩大信任面。出处：head.rs:60-74。
    pub fn rewrite_host(&mut self, target_port: u16) {
        let _ = target_port;
        todo!("W-TB")
    }

    /// hop-by-hop 取舍：非升级请求把 Connection 改为 close（一条浏览器连接对应
    /// 一条隧道流，响应终点 = 隧道 EOF）并剥 Keep-Alive/Proxy-Connection；
    /// WebSocket 升级请求原样保留 Connection/Upgrade（101 后是裸字节面）。
    /// 出处：head.rs:76-87。
    pub fn apply_hop_by_hop(&mut self, websocket: bool) {
        let _ = websocket;
        todo!("W-TB")
    }

    /// WebSocket 升级判定：Upgrade: websocket（大小写不敏感）且 Connection 含
    /// upgrade。出处：head.rs:89-95。
    pub fn is_websocket_upgrade(&self) -> bool {
        todo!("W-TB")
    }

    /// Content-Length（请求体转发量）；无头即 0，非法值为 Err。
    /// 出处：head.rs:97-106。
    pub fn content_length(&self) -> Result<u64, String> {
        todo!("W-TB")
    }

    /// 请求体是否 chunked（本反代不支持 chunked 请求体 → 501 拒）。
    /// 出处：head.rs:108-111。
    pub fn has_chunked_body(&self) -> bool {
        todo!("W-TB")
    }

    /// 序列化回 wire 字节（头尾 CRLF 完整）。出处：head.rs:113-126。
    pub fn to_wire(&self) -> Vec<u8> {
        todo!("W-TB")
    }
}

/// 读到 `\r\n\r\n` 为止的报文头；返回（原始头字节, 头后已到的 body 字节）。
/// 原始字节透传保证逐字保真（重序列化可能改变头行空格）；leftover 禁缓冲丢失；
/// 超 [HEAD_MAX] 显式报错（InvalidData），EOF 即 Err（UnexpectedEof）。
/// 出处：head.rs:129-156。
pub async fn read_head(
    stream: &mut (impl tokio::io::AsyncRead + Unpin),
) -> std::io::Result<(Vec<u8>, Vec<u8>)> {
    let _ = stream;
    todo!("W-TB")
}
```

### crates/p2p-tunnel/src/local_proxy/pump.rs（逐字）

```rust
//! 隧道数据泵导出面（契约 §1/§4 分块与半关闭 / §3.3 流式转发）：签名桩。
//! 请求体精确转发（bytesOut）与响应逐块排干（bytesIn）；流式转发禁整包缓冲；
//! 读侧容忍任意 ≤1 MiB 分块边界。实现 = W-TB（迁移自
//! apps/gui/src-tauri/src/tunnel/pump.rs；字节计数缝由 GUI ConnAudit 换为
//! [PumpAudit] trait，crate 内零 GUI 依赖）。

use std::time::Duration;

/// 隧道 IO 空闲护栏：响应/长连接期间读停滞超过该值即放弃（禁无限静默悬挂）。
/// 出处：apps/gui/src-tauri/src/tunnel/pump.rs:10-11。
pub const TUNNEL_IDLE_GRACE: Duration = Duration::from_secs(30);

/// 泵侧字节计数缝（§5.2 审计 bytes_in/bytes_out，记录方视角）。
/// 提炼自 apps/gui/src-tauri/src/tunnel/pump.rs 对 ConnAudit 的 add_in/add_out
/// 消费面（gui tunnel/audit.rs）；W-TB 迁移时由 ConnAudit 实现本 trait 或以
/// crate 内会话审计结构承载。
pub trait PumpAudit: Send + Sync {
    /// 自隧道收到 n 字节（响应方向）。
    fn add_in(&self, n: u64);
    /// 向隧道发出 n 字节（请求方向）。
    fn add_out(&self, n: u64);
}

/// 读应答头原始字节（带 [TUNNEL_IDLE_GRACE] 护栏）+ 头后已到的 body 字节。
/// 超时/IO 错误为人话 Err。出处：apps/gui/src-tauri/src/tunnel/pump.rs:13-21。
pub async fn read_reply_head(
    stream: &mut (impl tokio::io::AsyncRead + Unpin),
) -> Result<(Vec<u8>, Vec<u8>), String> {
    let _ = stream;
    todo!("W-TB")
}

/// 应答首行是否 101 升级协议切换（HTTP/1.0 或 HTTP/1.1）。
/// 出处：apps/gui/src-tauri/src/tunnel/pump.rs:23-27。
pub fn is_101(raw: &[u8]) -> bool {
    let _ = raw;
    todo!("W-TB")
}

/// 请求体精确转发：leftover 先落，再补足 Content-Length 差额；逐块计
/// bytesOut；body 在 Content-Length 之前 EOF 即 Err。
/// 出处：apps/gui/src-tauri/src/tunnel/pump.rs:29-61。
pub async fn forward_exact(
    src: &mut (impl tokio::io::AsyncRead + Unpin + Send),
    dst: &mut (impl tokio::io::AsyncWrite + Unpin + Send),
    leftover: Vec<u8>,
    content_len: u64,
    audit: &dyn PumpAudit,
) -> Result<(), String> {
    let _ = (src, dst, leftover, content_len, audit);
    todo!("W-TB")
}

/// 隧道 → 本地逐块排干（流式，禁整包缓冲）；EOF 即响应终点并关本地写半；
/// 逐块计 bytesIn；读停滞超 [TUNNEL_IDLE_GRACE] 即 Err。
/// 出处：apps/gui/src-tauri/src/tunnel/pump.rs:63-94。
pub async fn drain_reply(
    tunnel: &mut (impl tokio::io::AsyncRead + Unpin),
    local: &mut (impl tokio::io::AsyncWrite + Unpin),
    first: Vec<u8>,
    audit: &dyn PumpAudit,
) -> Result<(), String> {
    let _ = (tunnel, local, first, audit);
    todo!("W-TB")
}
```

### crates/p2p-tunnel/src/lib.rs 冻结 re-export 面（逐字，fmt 后形态）

```rust
pub use local_proxy::head::{read_head, Head, HEAD_MAX};
pub use local_proxy::pump::{
    drain_reply, forward_exact, is_101, read_reply_head, PumpAudit, TUNNEL_IDLE_GRACE,
};
pub use local_proxy::{LocalProxy, ProxyCtx, TunnelOpener};
```

（lib.rs 另有改动：`mod local_proxy;` 挂入模块表 + crate 级 doc 注释更新为
「访侧本地反代核心落 local_proxy 模块（W-TA 签名桩）」口径，逐字见 b2952fbc。）

## 给 TB 的移交清单

1. 填全部 `todo!("W-TB")`（机械平移 GUI 三文件实现体），填毕移除各 `#[allow(dead_code)]`；
   GUI proxy_tests.rs 一并迁 crate（ManualOpener/DeadOpener 即 TunnelOpener 测试缝）。
2. GUI visit.rs 的 NodeTunnelOpener 改 impl crate 的 TunnelOpener（签名逐字同源，预期零阻力）；
   apps/gui 切换随 TB 提交。
3. tunnel_status 的 sessions 组装：TunnelAuditRecord → TunnelSessionAudit 的 camelCase
   映射落 GUI 装配层；outcome 映射对齐 §19.3-5 六值闭集 + "open"。
4. new_uid 的 getrandom 依赖留在 GUI 装配侧传入或 TB 决断 crate 内替代（uid 语义 =
   16 hex，§5.2）。
5. TC（p2pctl tunnel connect）直接消费 TunnelOpener + LocalProxy + HEAD_MAX/Head/pump
   导出面；TD（GUI 通用入口）消费 tunnel_open 契约条款（gui-contract §19.1 表行 +
   §19.3-9）。

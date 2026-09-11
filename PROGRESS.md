# PROGRESS — TA：tunnel 泛化契约冻结 + 访侧反代核心签名桩

分支 feat/wta-tunnel-contract ｜ worktree .worktrees/wta-contract ｜ 基于 origin/main @ a9ea63e1
类型 architecture ｜ 状态：DONE（待全量 workspace check 收口 + push）

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
7. [x] `cargo check --workspace` EXIT=0（后台跑批收口）
8. [x] push origin feat/wta-tunnel-contract（禁自合并，合并归协调者）

## 提交拆分（桩与登记类分离）

- b2952fbc feat(tunnel): local_proxy 访侧反代核心签名桩（W-TA 契约先行）
- docs(design): gui-contract §19 —— tunnel_open 通用命令 + §19.8 connect 对等 exempt（独立小提交）
- docs(protocol): specs/tunnel.md §8 实现状态口径与漂移登记（独立小提交）
- docs(orchestrator): PROGRESS.md（本文件）

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

lib.rs re-export 面（冻结导出）：

```rust
pub use local_proxy::head::{read_head, Head, HEAD_MAX};
pub use local_proxy::pump::{
    drain_reply, forward_exact, is_101, read_reply_head, PumpAudit, TUNNEL_IDLE_GRACE,
};
pub use local_proxy::{LocalProxy, ProxyCtx, TunnelOpener};
```

local_proxy/mod.rs：

```rust
pub mod head;
pub mod pump;

#[async_trait::async_trait]
pub trait TunnelOpener: Send + Sync {
    async fn open(&self, uid: &str, target: &str) -> Result<TunnelIo, TunnelError>;
}

pub struct ProxyCtx { /* target_port: u16; peer_id: String; opener: Arc<dyn TunnelOpener>; conns: AtomicU32; audit: TunnelAudit（均私有） */ }
    pub fn active_conns(&self) -> u32;
    pub async fn audit_snapshot(&self) -> Vec<TunnelAuditRecord>;

pub struct LocalProxy {
    pub ctx: Arc<ProxyCtx>, /* listener: tokio::net::TcpListener（私有） */
}
    pub async fn bind(target_port: u16, peer_id: String, opener: Arc<dyn TunnelOpener>) -> std::io::Result<Self>;
    pub fn local_addr(&self) -> SocketAddr;
    pub async fn serve(self);
```

local_proxy/head.rs：

```rust
pub const HEAD_MAX: usize = 32 * 1024;

pub struct Head {
    pub first_line: String,
    pub headers: Vec<(String, String)>,
}
    pub fn parse(bytes: &[u8]) -> Result<Self, String>;
    pub fn header(&self, name: &str) -> Option<&str>;
    pub fn set_header(&mut self, name: &str, value: &str);
    pub fn rewrite_host(&mut self, target_port: u16);
    pub fn apply_hop_by_hop(&mut self, websocket: bool);
    pub fn is_websocket_upgrade(&self) -> bool;
    pub fn content_length(&self) -> Result<u64, String>;
    pub fn has_chunked_body(&self) -> bool;
    pub fn to_wire(&self) -> Vec<u8>;

pub async fn read_head(
    stream: &mut (impl tokio::io::AsyncRead + Unpin),
) -> std::io::Result<(Vec<u8>, Vec<u8>)>;
```

local_proxy/pump.rs：

```rust
pub const TUNNEL_IDLE_GRACE: Duration = Duration::from_secs(30);

pub trait PumpAudit: Send + Sync {
    fn add_in(&self, n: u64);
    fn add_out(&self, n: u64);
}

pub async fn read_reply_head(
    stream: &mut (impl tokio::io::AsyncRead + Unpin),
) -> Result<(Vec<u8>, Vec<u8>), String>;
pub fn is_101(raw: &[u8]) -> bool;
pub async fn forward_exact(
    src: &mut (impl tokio::io::AsyncRead + Unpin + Send),
    dst: &mut (impl tokio::io::AsyncWrite + Unpin + Send),
    leftover: Vec<u8>,
    content_len: u64,
    audit: &dyn PumpAudit,
) -> Result<(), String>;
pub async fn drain_reply(
    tunnel: &mut (impl tokio::io::AsyncRead + Unpin),
    local: &mut (impl tokio::io::AsyncWrite + Unpin),
    first: Vec<u8>,
    audit: &dyn PumpAudit,
) -> Result<(), String>;
```

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

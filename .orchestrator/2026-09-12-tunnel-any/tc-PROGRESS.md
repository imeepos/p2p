# PROGRESS — TC：p2pctl tunnel connect（headless 访侧）+ 双节点 itest

分支 feat/wtc-cli-connect ｜ worktree .worktrees/wtc-connect ｜ 基于 main @ 96514234
类型 code-quality ｜ 预算 120 次调用（40/80/120 分段回报）｜ 状态：DONE

## 分段进度

1. [x] 读 AGENTS.md / 任务书 / serve.rs（形态模板）/ ta-PROGRESS 冻结面 /
   local_proxy 现实现（mod/conn/session/head）/ visit.rs NodeTunnelOpener /
   tb-PROGRESS「给 TC 的移交注记」/ ai-docs-sync.sh 机械口径
2. [x] 建树：fetch origin 反向同步（main==origin/main @ 96514234）→
   `git worktree add .worktrees/wtc-connect -b feat/wtc-cli-connect main`
3. [x] apps/cli/src/tunnel/connect.rs：ConnectArgs（--peer/--target 必填 +
   data_dir/quic_port/tcp_port/no_mdns/bootstrap 与 serve 逐项同名同默认，
   无 --listen）/ build_config 先校验后动作 / run（前台常驻 + SIGINT/SIGTERM
   → abort serve 任务 → drain active_conns 超时非零退出 → stopped 行）/
   NodeTunnelOpener（uuid v4 作 nonce 源）+ 内嵌 4 单测
4. [x] itest：tests/tunnel_connect_common/{mod,ws}.rs（真实回环 HTTP 固定
   body 服务 + RFC6455 WS echo 服务 + ConnectOpener + 双节点 rig + 访侧
   start_visit_proxy + SHA-1/base64 自足实现）+ tests/tunnel_connect_wave.rs
   （g1 HTTP GET 全链 + g2 WS 升级/accept 校验/文本与二进制帧双向 echo/close
   收口/审计终态）
5. [x] 登记：docs/ops/p2pctl-ai-guide.md tunnel 域 connect 条目（ai-docs-sync
   参数机械比对过，含 --peer 示例用真 base58 串）
6. [x] 门禁七条全绿（证据见下）
7. [x] 提交拆分（style 存量漂移独立提交）+ push origin feat/wtc-cli-connect
   （禁自合并，合并归协调者）

## 验收证据（命令 + 真实退出码，worktree 内执行）

| 门禁 | 命令 | 结果 |
|---|---|---|
| 1 | cd apps/cli && cargo test | EXIT=0（158 passed，含 connect 单测 3 条） |
| 2 | cargo test -p p2p-itest --test tunnel_connect_wave | EXIT=0（4 passed = g1+g2+SHA-1/accept RFC 向量 ×2） |
| 3 | cargo clippy --all-targets -- -D warnings（apps/cli 独立 workspace） | EXIT=0 |
| 3' | cargo clippy -p p2p-itest --all-targets -- -D warnings | EXIT=0 |
| 4 | cargo fmt --check（apps/cli 与根 workspace 双树） | EXIT=0 双绿 |
| 5 | bash scripts/check/tests/panic-hygiene.sh | EXIT=0（11 通过，自测 PASS） |
| 6 | bash scripts/check/ai-docs-sync.sh | EXIT=0（AI-DOCS-OK：叶子 101/条目 101/参数比对 347 项/示例抽验 6 条） |
| 7 | git diff main --name-only | 全部落在 apps/cli/ + crates/p2p-itest/ + docs/ops/p2pctl-ai-guide.md + .orchestrator/2026-09-12-tunnel-any/（根 Cargo.lock 零改动，见裁定 3） |

## 关键裁定（后续卡必读）

1. **opener nonce 源**：apps/cli 无 getrandom，用既有 uuid v4 的 16 字节
   hex 编码（32 hex 满足 wire.rs 校验）；不为此引新依赖。
2. **itest 访侧直连装配**：apps/cli 是独立 cargo 项目仅 bin 目标，itest 无法
   path 依赖；测试以同源 crate 冻结面（LocalProxy + TunnelClient +
   TunnelOpener + NodeFactory 裸流工厂）镜像 connect.rs 装配——非子进程，
   满足任务书。connect.rs 改动时须同步该镜像（tb-PROGRESS 同款注记）。
3. **零锁文件改动**：WS accept 需 SHA-1，workspace 无 sha1 crate；裁定在
   测试夹具内自足实现 SHA-1 + base64（RFC 3174/6455 §1.3 标准向量锚定），
   不给 p2p-itest 加 ring/base64 dev-dep——避免根 Cargo.lock 改动越出本卡
   文件范围（曾试装依赖并跑绿，为守验收第 7 条回退）。
4. **双目标 = 双 LocalProxy 实例**：LocalProxy 一实例一 target；B 侧对
   HTTP/WS 两白名单各起一个反代（镜像两条 connect 进程并存）。
5. **ready 行 peerId = 被访节点**（与 LocalProxy 审计字段同源）；stopped 行
   三态计数跳过 ended_at==0 存续期哨兵记录（tb-PROGRESS 漂移①口径）。

## 踩坑（self-evolving 喂回项）

- **cargo 不在默认 PATH + 管道收尾吃退出码**：首验 `cargo check | tail` 得
  假 EXIT=0（实为 tail 的，cargo 是 command not found）；复跑一律
  `PATH=$HOME/.cargo/bin` 前置 + 退出码独立落文件再读。
- **升级路径会话收口以本地半关为前提**：WS close 帧往返后测试侧仍持
  socket，双向对称泵不结束、审计不落终态；`sock.shutdown()` 发 FIN 后才
  收口。HTTP 面因 plain_path 转发完请求即只余 drain 方向，无此问题。
- **unwrap_err 需 T: Debug**：ConnectConfig 手写 Debug（PeerId 无 Debug
  派生，p2p-identity 属 panic-hygiene 保护 crate 不动）。
- **clippy 新 lint chunks_exact_to_as_chunks**：常量 chunk 用
  `as_chunks::<N>()`。
- **main 存量 fmt 漂移**：apps/cli a2a/cli/main/report 7 文件在 main 上即
  不达 cargo fmt --check；本卡 `cargo fmt` 会连带格式化，已回退本卡外文件
  并把漂移修复压成独立 style 提交（可独立 revert，不混入 feature）。

## 给 TE 的移交（跨机 e2e 证据卡）

1. 两机各起一节点（同 LAN 缺省 mDNS 自动发现；跨网段两侧加同一
   --bootstrap）。
2. 被访机 A：`p2pctl tunnel serve --target 127.0.0.1:<P>`（stdout ready 行
   取 peerId）；被访机 <P> 上跑任意 HTTP/WS 服务（如 python3 -m http.server）。
3. 访机 B：`p2pctl tunnel connect --peer <A_PEER> --target 127.0.0.1:<P>`；
   ready 行 localAddr 即本地入口——`curl http://<localAddr>/` 应得 A 机 <P>
   服务响应；WS 客户端直连 `ws://<localAddr>/` 应完成升级与双向帧。
4. e2e 断言锚点：ready/stopped JSON 行字段（localAddr/target/peerId/
   sessions/served）；收口对 connect 进程发 SIGTERM（timeout N 同效），退出
   码 0 = 优雅收口，1 = 收口超时（在途未落终态）。
5. 白名单外 target：A 拒绝经错误帧透传，B 反代侧连接以 HTTP 5xx 短应答
   收口（conn.rs reject 面），不悬挂。

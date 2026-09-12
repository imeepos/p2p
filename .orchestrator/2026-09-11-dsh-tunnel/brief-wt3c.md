# W-T3c：隧道反代 Origin/Referer 重写修复（红绿小卡）

## 背景与根因（已判别实验定案，勿重复诊断）
W-T3b 全链 e2e（.orchestrator/2026-09-11-dsh-tunnel/wt3b/EVIDENCE.md，必读）：反代只重写
Host、未重写 Origin/Referer → 浏览器 `Origin=http://127.0.0.1:<proxyPort>` 被 DSH /api
authority 栅栏 403（GET 过、POST/WS 拒；同浏览器直连 3080 全 200）。修复属 p2p 侧，
合乎形态 A「DSH 零改动」裁定。

## 范围（文件域独占，与并行卡 W-T4 互斥）
- 只许改：`apps/gui/src-tauri/src/tunnel/**`（head.rs 重写面 + proxy 传递 + 单测）、
  `docs/protocol/specs/tunnel.md` 中冻结重写面的那节（Host 重写处扩 Origin/Referer，
  契约变更随码同提交）、`docs/design/gui-contract.md` 若有对应措辞处。
- 禁改：crates/**、apps/cli/**、scripts/check/**（W-T4 域）；不改 generate_handler!（无新命令）。

## 修法要求
- Origin/Referer 存在且 host 为回环字面量时重写为 `http://127.0.0.1:<target_port>`
  （与 Host 重写同目标 authority）；无该头不造头。安全边界写进注释：重写目标仅限
  票据 target 的回环 authority，不扩大信任面。
- WS 升级请求路径同样覆盖（Upgrade 头存在时 Origin 同步重写）。

## 红绿验收（按用户红绿双向标准，缺一不收）
1. 红：head.rs 单测先行——Origin/Referer 重写断言在修复前必红（红证据留档）。
2. 绿：单测 + proxy_tests 覆盖 POST 报文与 WS 升级请求两路径均重写后过栅栏。
3. 全链：复跑 W-T3b 注入驱动链（.orchestrator/2026-09-11-dsh-tunnel/wt3b/ 的
   driver-proxy.mjs + driver.js，/tmp/wt3b-* 复跑环境尚在）把 EVIDENCE 硬项 #4 翻
   PASS：remote.mux WS 握手 200/101 + 一次真实流式对话 + /api 全 200；补浏览器面
   截图（注意控制通道 /screenshot 帧源滞后缺陷，用 DOM 文本+网络日志为主证）。
4. 门禁：pnpm lint/test/build/check:i18n 全绿；src-tauri cargo test + `cargo clippy
   --all-targets -- -D warnings`；bash scripts/check/cli-parity.sh 末行 CLI-PARITY-OK。

## 纪律
- worktree 自建（fetch 后 rebase 最新 main）；分支名 feat/wt3c-origin-rewrite；
  push origin，禁自合并（协调者验收合并）。
- 预算检查点分段：①红证据 ②单测绿 ③全链翻 #4 ④门禁——每段末回报一次；
  总顶 100 次调用，段末超支提前预警。
- 发现 W-T4 域问题只记录不动手。完成写 DONE 报（含证据指针）回报派发者。

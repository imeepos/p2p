# Orchestrator 计划 — 2026-09-11 收口轮（CI 红 + 互操作规范缺口）

协调会话：session-3373f897-f9c0-4a32-9af2-b9562eecc311（本会话，单主会话，无并存 lead）
基线：main @ e91119e（== origin/main），工作树唯一未跟踪项 = `.agents/skills/dev-orchestrator/`（本轮 W3 处置）
事实源：AGENTS.md / docs/coordination.md（末条 09-09 I2 收官）/ ISSUE.md / examples/mininode-python/SPEC-GAPS.md / git log

## 0. 事实核查（Facts，附证据）

- F1 **main CI 三连红且非平台偶发**：GitHub `ci` run 34472747421(4212ea8)/34475371238(8497b2f)/34477498790(e91119e) 全 failure，
  失败步唯一集中在「全量门禁」（其余步全 success）。证据：`api.github.com/repos/imeepos/p2p/actions/runs/.../jobs`。无 admin 权限，job 日志拉取 403。
- F2 **本地等价复现成功**：`export PATH=$HOME/.cargo/bin:$PATH; CI=true make check` → `Makefile: gate-tests Error 1`，
  `scripts/check/tests/affected-fast.sh: line 32: name<乱码>: unbound variable`，`MAKE_CHECK_EXIT=2`。日志 /tmp/local_make_check.log。
- F3 **该自测单独跑稳定绿**：`bash scripts/check/tests/affected-fast.sh` 连跑 4 次（bash 5.3.9 与 /bin/bash 3.2.57 各若干）全 `pass=20 fail=0`；
  `CI=true make gate-tests` 单独跑也绿。→ 只在 make check 全量编排下复现，属编排/负载相关确定性问题（不是随机 flake）。
- F4 **GAP 修订从未落地**：`docs/protocol/specs/rendezvous.md` 仍只写「一帧一个消息，长度前缀封装」，
  `docs/protocol/wire-format.md §6` 仍宣称流上全部帧为 varint 前缀；而实现 `crates/p2p-discovery/src/rendezvous/link.rs:159`
  用 `(data.len() as u32).to_be_bytes()`。第三方按文档实现必翻车（IV1 黑盒实证：服务端报 frame size too big 断开）。
- F5 缺口余项同样未落地：GAP-3 ping 应答方关流方向、GAP-4 PeerMismatch 断开时机、GAP-5 默认 namespace `p2p-base`
  （实现 `crates/p2p/src/assembly.rs:25 DEFAULT_NAMESPACE`）、GAP-6 半帧悬挂超时。
- F6 **a2a Tauri 命令是 5 个未接线空壳**：`apps/gui/src-tauri/src/a2a/commands.rs` 五个 `#[command]` 直接返回桩值，
  前端零调用点（`apps/gui/src` grep 无 `a2a_list|a2a_publish|...`）；真实数据经 a2a 宿主 admin HTTP 走（`views/agents/*`）。
- F7 发布面无缺口：client-v0.1.7 tag 已推送且 release 已 published（2026-09-10T12:28:51Z，draft=false）；gui-client 工作流 main 上 success。
- F8 工作区仅本会话一个（`session_link_list`），无多 lead 并存，无登记表冲突。

## 1. 波次（锁定区）

| 卡 | 类型 | 目标 | 依赖 |
| --- | --- | --- | --- |
| W1 CI 红收口 | quality-gates | 定位 affected-fast 在 make check 编排下必红根因并修到绿，附红绿双向证据 | 无（先于一切合并） |
| W2 GAP 规范回写 | architecture | 六条 GAP 按「以代码为准」回写 docs/protocol/**，消除第三方接入阻断 | 无（与 W1 零文件交集） |
| W3 skill 入库 + 账本收口 | code-quality(规则治理) | dev-orchestrator skill 落盘入库，确认无本地化内容泄漏 | 无（协调者直做小提交） |

W1 与 W2 文件域零交集（scripts/check/** vs docs/protocol/**），可并行派发；W3 由协调者自做。

## 2. 待细化区（下一波候选，不在本轮承诺）

- ACP1 = ISSUE.md「Phase2 acp-agent 收口 p2pctl」：现状 `p2pctl acp` 有 console/status/allow/deny/list/share，无 agent 前台子命令；
  细化触发条件 = 本轮两卡合并完成且用户确认启动新波。
- ACP2 = 「Phase3 pump 单节点化」：前置是 p2p facade 开流 API（宿主以已有 Node 直接 register handler/拨流）；
  细化触发条件 = facade API 设计卡定稿。
- A2A6 = 真 dsh #[ignore] itest 与 SKIP 信号（A2A 波显式降级登记第 4 条）；细化触发条件 = ACP 收口波完成（acp-agent 形态变更会重排该夹具）。
- DEBT = a2a 空壳命令摘除（F6）+ 磁盘/验收 target 清扫守卫；细化触发条件 = 有 GUI IPC 面改动波时随卡处理。

## 3. 计划版本

- Plan v1 | 首版 | 依据 F1-F8 | 审批级：本波次范围（W1-W3 均为收口/回写，无契约变更）

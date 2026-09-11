# W-T3c DONE 报（2026-09-11）

## 结论：DONE —— 硬项 #4 已翻 PASS，门禁全绿，分支已 push，待协调者验收合并

## 交付
- 分支 `feat/wt3c-origin-rewrite`（基于 main e7229f7a，worktree ../p2p-wt3c，未自合并）
- 提交链：
  - 87a96701 test(gui-tauri): Origin/Referer 重写断言先行（红证据 wt3c/RED.md）
  - 3635fb47 fix(gui-tauri): 反代 Origin/Referer 重写为目标回环 authority（契约 §2/§3.2/§3.3 + gui-contract §19.2 随码同步）
  - 2f9002a7 docs(orchestrator): 段③全链复跑证据
  - d596bb88 chore(gui-tauri): 移除 serve_conn 遗留调试输出

## 修法（一句话）
`Head::rewrite_host` 扩为同面重写：Origin/Referer 存在且 authority host 为 `127.0.0.1`
回环字面量（与 parse_dsh_url 同判据）→ 重写为票据 target 的同一 authority；
无头不造头；`null`/localhost 原样保留；重写目标仅限票据 target，不扩大信任面。
WS 升级与 POST 同走 forward_conn 单一重写点，天然双覆盖。

## 红绿证据
- 红（修复前）：`cargo test --lib tunnel::` = 15 passed / 4 failed（4 断言全红，
  报文原文显示 Host 已重写而 Origin/Referer 仍为反代端口）→ wt3c/RED.md
- 绿（修复后）：tunnel:: 19/19；全库 151/151
- 全链（wt3b 驱动链复跑，代码侧为修复版）：wt3c/EVIDENCE-wt3c.md
  - /api POST 全 200（wt3b 全 403）；remote.mux WS 握手 101（3ms）；真实流式对话
    「1+1等于2。」（kimi-k3，7.8K tok，58 tok/s）；审计 225 会话，bytesIn≈12.2MB；
  - 截图 wt3c-full-chain-chat.png（浏览器面）

## 门禁（全绿）
| 项 | 结果 |
|---|---|
| pnpm lint（main 树，前端零改动的等价树） | PASS |
| pnpm test | 1408 passed |
| pnpm build | built 11.28s |
| pnpm check:i18n | PASS（zh=1609 en=1609） |
| cargo test --lib（src-tauri，worktree） | 151 passed / 0 failed |
| cargo clippy --all-targets -D warnings（clippy.sh） | PASS |
| bash scripts/check/cli-parity.sh | 末行 CLI-PARITY-OK |

## 遗留与移交
1. **busy 栅栏并发上限（W-T4 域，未动手）**：crates/p2p-tunnel serve gate 默认
   `max_concurrent: 4`；浏览器并行 burst 触发 busy→502，页面需刷新 1-2 次。
   建议后续卡裁决（调上限 / 代理侧重试 / 文档化刷新语义）。
2. 复跑环境操作（未改仓库）：DSH_HOME 换 /tmp/wt3c-dsh-home（旧 home 持久化了 wt3b
   反代地址 52172，页面 mux 指死端口）；driver-proxy 运行副本绑双栈。
3. 环境已清理：双 GUI/DSH/vite/driver-proxy 全部退出，gui-config.json 恢复备份。
   worktree ../p2p-wt3c 保留待合并后移除。

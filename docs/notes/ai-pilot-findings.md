# AI 操作者试运行 findings（p2pctl-ai-guide.md 可操作性检验）

- 日期：2026-09-04；试运行者：AI 操作者（仅依据 docs/ops/p2pctl-ai-guide.md 操作，未读任何源码）
- 项目根：/Users/imeepos/ext512/p2p；隔离数据目录 /tmp/ai-pilot-a、/tmp/ai-pilot-b，结束已清理
- 结论速览：S1/S2/S3/S6 PASS，S4 部分 PASS（screenshot 失败），S5 FAIL —— 4.5/6

## ① 执行结果总表

| 步骤 | 结论 | 关键命令 | 说明 |
|---|---|---|---|
| S1 构建 | PASS | `export PATH=$HOME/.cargo/bin:$PATH && cargo build --manifest-path apps/cli/Cargo.toml` | 首跑 `cargo: command not found`（摩擦 F1）；产物 apps/cli/target/debug/p2pctl v0.1.0 |
| S2 双节点 | PASS | `p2pctl node start --data-dir /tmp/ai-pilot-a --json`（B 同理）+ `node status --json` ×2 | A pid=81444 守护peer=aogbzDcMk5VeRUVkjLK8kLHHv4FWbaeQkg57ErxKmcq（127.0.0.1/u52063,t59667）；B pid=81450 守护peer=HrEjjf4G1bGWxEYRNMnqnM5TJT1rsmHSpiifCMuxMaCy（127.0.0.1/u53264,t59669）；双 running=true |
| S3 好友+收发 | PASS（第 3 次尝试才通） | 见 §附A 失败记录 F-a/F-b | 正解拓扑：B 起 `chat serve`；A `chat friends add <B chat peerId> --addr 127.0.0.1/u57844 --addr 127.0.0.1/t59850`；A `chat send` → delivered=true（id=0a4fdac8-410c-483c-9cf6-bb007fa4a814）；B `chat history --peer <A chat peerId>` 读回同一 id、sender=them、文本一致，断言通过 |
| S4 GUI | 部分 PASS | `gui status --json` / `gui page --json` / `gui navigate chat --json` 全过 | `gui screenshot --output /tmp/ai-pilot-s4.png --json` FAIL：CAPTURE_PERMISSION_DENIED（摩擦 F6），PNG 未产出 |
| S5 UI 回归 | FAIL | `bash scripts/ops/ui-regression.sh` | 末行 `UI-REG-FAIL 失败页: chat dashboard diagnostics discovery events peers relay settings`（非 UI-REG-OK）；8/8 路由均只挂在 screenshot 步骤，其余断言 58/74 通过，根因同 F6 |
| S6 清理 | PASS | `node stop` ×2、`rm -rf /tmp/ai-pilot-a /tmp/ai-pilot-b /tmp/ai-pilot-s4.png` | 复核：双节点 running=false、pgrep 无 ai-pilot 残留、临时目录已删；GUI（pid 38028，先于本试运行运行）按剧本未杀；另一会话的 chat serve（pid 17651，/tmp/p2p-r8-a）非本试运行产物，未动 |

## ② 摩擦清单

| # | 卡点 | 文档缺口 | 建议补法 | 严重度 |
|---|---|---|---|---|
| F1 | `cargo build` 首跑报 command not found | 构建命令假设 cargo 在 PATH，无前置说明 | §开头补一行：需 cargo 在 PATH（本机 ~/.cargo/bin） | 低 |
| F2 | S3 最大卡点：不知道 chat 收发用哪套 peerId、接收方要跑什么。用守护 peerId + 守护地址加好友/发送 → 快速失败；靠报错与 §1.3「两套身份根」猜测出：接收方须 `chat serve` 常驻，好友记录须用 **chat 身份** peerId + chat serve 监听地址（猜测行为即摩擦） | 文档只陈述「chat serve 与 node start 的 peerId 可以不同」的现象，没有「两节点聊天最小拓扑」配方；`chat friends add --addr` 的格式示例也只在 peer dial 条目出现 | 补一节「两节点聊天 E2E 最小拓扑」：每端跑什么（chat serve）、peerId 取自哪条命令输出、--addr 怎么填、chat send 是否需要本机守护进程 | 高 |
| F3 | A 侧起 chat serve（为拿 chat 身份）后，同 data-dir `chat send` 报「身份被占用…锁=/tmp/ai-pilot-a/identity.lock」；node 守护与 chat serve 却可共存——互斥关系只能试出来 | §1.3 守护信号只列 daemon.pid/meta/sock/log；identity.lock 完全未记载，锁的持有者（chat serve）与需求方（chat send）关系未写 | §1.3 补锁文件清单与互斥矩阵（谁持锁/谁等待/如何安全释放），排障指引写明「停掉同数据目录另一 chat 进程即可」 | 高 |
| F4 | `chat send` 失败形态文档只写「超时未送达 status=Pending」；实测对端身份不符时**立即** status=Failed（exit 1），且无排查线索 | 缺 Failed 语义（快速失败：对端不可达/身份不匹配）及与 Pending 的区分 | chat send 条目补 status=Failed 形态、典型成因（对端 peerId 填了守护身份而非 chat 身份） | 中 |
| F5 | 本机 chat 身份 peerId 无只读查询命令；只能临时起 `chat serve` 读首行才知道，而它又占 identity.lock，学完还得停掉才能 send（自相矛盾的流程） | 45 命令无「查本机 chat 身份」条目 | 补 `chat identity show`（或让 node status/chat 某读命令暴露 chat peerId），离线可读不占锁 | 中 |
| F6 | `gui screenshot` 与 S5 整条 ui-regression 都挂在 macOS 屏幕录制权限：`[CAPTURE_PERMISSION_DENIED] …（HTTP 403）`；OS 级授权 AI 无法自助完成，PNG 出不来，S5 末行永远到不了 UI-REG-OK | §1.4 前置矩阵与 gui screenshot 条目只写「GUI 进程运行；路径须绝对」，未提屏幕录制权限 | 前置矩阵加一行「macOS 屏幕录制授权（gui screenshot/record、ui-regression.sh）」+ CAPTURE_PERMISSION_DENIED 释义与授权路径 | 中 |
| F7 | gui page 文档说「当前页未在注册表（如 dashboard）时 PAGE_NOT_REGISTERED 拒绝」，实测当前页 dashboard 时 `gui page --json` 正常返回 descriptor（含 start/stop 动作） | 文档示例与实现行为不一致 | 更新 gui page 条目：dashboard 已注册（或删去该反例） | 低 |
| F8 | `node status --json` 文档只给 running=false 形态，running=true 形态（含 peerId/listenAddrs）无示例，首用需自行试探 | 输出示例不全 | 补 running=true 的 JSON 示例 | 低 |

## ③ 文档评分

**7 / 10** —— 45 条命令的参数/退出码/双形态输出示例覆盖扎实，读操作零摩擦上手快；扣分在三个操作性盲区：聊天两节点拓扑（chat 身份 vs 守护身份的实际用法）、identity.lock 互斥、screenshot 的 OS 权限前置，恰好让 S3/S4/S5 首试全部卡住。

## ④ 剧本是否全部完成

**否（4.5/6）**：S1/S2/S3/S6 通过；S4 的 screenshot 与 S5（末行 UI-REG-FAIL，非 UI-REG-OK）失败，两者同根因——macOS 屏幕录制权限未授予 GUI，需人在「系统设置 > 隐私与安全性 > 屏幕录制」中授权，AI 无法自助完成；授权后按本文档其余部分应可直接重跑通过。

## 附A：失败原样记录（命令 + 完整报错）

- F-a（S3 首试，守护 peerId 当 chat 对端）：
  `p2pctl chat send --peer HrEjjf4G1bGWxEYRNMnqnM5TJT1rsmHSpiifCMuxMaCy --text "ai-pilot-S3-hello-from-A" --data-dir /tmp/ai-pilot-a --json`
  exit 1；stdout `{"message":{...,"status":"failed",...},"delivered":false,"flushedOutbox":0}`；stderr `p2pctl: 运行失败: 消息未送达对端（status=Failed），已保留本机记录`（B 端 history 为空 `[]`）
- F-b（同 data-dir chat serve 与 chat send 互斥）：
  `p2pctl chat send --peer 7aUVzmkJwDx9HVnWcNDqa6QNSAuuRBAwLLMzirFpcMqP … --data-dir /tmp/ai-pilot-a --json`
  exit 1；stderr `p2pctl: 运行失败: 身份被占用：该身份已有进程在运行（同数据目录不支持多程序并行），如需切换请先停止另一进程；锁=/tmp/ai-pilot-a/identity.lock，好友簿写锁 /tmp/ai-pilot-a/identity.lock 等待 0ns 未获取：并发写者僵持或残留陈锁，拒绝静默覆盖`
- F-c（S4.4 截图）：
  `p2pctl gui screenshot --output /tmp/ai-pilot-s4.png --json`
  exit 1；stderr `p2pctl: 运行失败: [CAPTURE_PERMISSION_DENIED] macOS 屏幕录制权限缺失：请在 系统设置 > 隐私与安全性 > 屏幕录制 中授权 GUI 后重试（HTTP 403）`
- F-d（S5 回归）：
  `bash scripts/ops/ui-regression.sh` exit 1；末行 `UI-REG-FAIL 失败页: chat dashboard diagnostics discovery events peers relay settings（原因见上表 note 列）`；SUMMARY `pages=8 passed=0 failed=8 assertions=58/74`；各路由 note 均为 gui screenshot CAPTURE_PERMISSION_DENIED
- F-e（S1 首跑）：`cargo build --manifest-path apps/cli/Cargo.toml` → `bash: cargo: command not found`（exit 127）

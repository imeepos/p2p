# 远程支持单 runner 接入（repair-bridge ⇄ repair-helper）

> 基线 main @ 4fe16ec（2026-09-03）| 对应 docs/design/remote-support-design.md §9「任意 agent 接入」、
> docs/design/remote-support-plan.md §2（T28）。定位：P0b 交付物五——单 runner 接入说明，
> DSH 为接入首例，Codex / Claude Code 与 DSH 同 stdio 桥原理同构（见 §8）。

## 1. 端到端拓扑与角色

客户机（被维修电脑）不运行任何 AI：repair-helper serve 起一个临时 MCP server，
只暴露 P0b 工具面；服务侧任意 agent（runner）经本地 stdio 桥 repair-bridge 拨入，
桥把 MCP JSON-RPC 字节与 p2p 流双向对拷，不解析 MCP 语义（design §6）。

| 角色 | 进程 | 位置 | 职责 |
|---|---|---|---|
| runner | DSH / Codex / Claude Code | 服务侧 | 经 mcpServers 挂桥，驱动工具面 |
| 接入桥 | repair-bridge | 服务侧 | stdio ⇄ p2p 流双向哑泵（T20） |
| 维修助手 | repair-helper | 客户机 | 临时 MCP server + 本地执法 + 审计（T21/T23/T23b/T26） |
| bootstrap | p2p-cli bootstrap | 公网 138/ECS | rendezvous 注册/查询，桥与助手互发现 |

调用链路：DSH --stdio--> repair-bridge --p2p /repair/mcp/1--> repair-helper。

## 2. 接入前置

```sh
export PATH="$HOME/.cargo/bin:$PATH"
cargo build --release -p repair-bridge -p repair-helper
```

- bootstrap 地址（ip/u端口 为 QUIC、ip/t端口 为 TCP）：138 43.240.223.138/u3400；
  ECS 121.196.193.177/u3400（docs/ops/experiment-env.md §8.5）。
- 测试平台密钥：P0b 与 itest 同源（crates/p2p-itest/tests/common/mod.rs platform()），
  种子文件为 32 字节 0x07（printf "\x07" 重复 32 次），
  对应平台公钥 hex ea4a6c63e29c520abef5507b132ec5f9954776aebebe7b92421eea691446d22c。
  生产由调度签发正式票据，属 P1（plan §3.3）。

## 3. 客户机侧：启动 repair-helper serve

```sh
REPAIR_ROOTS=/tmp/repair-root \
repair-helper serve \
  --bootstrap 43.240.223.138/u3400 \
  --data-dir ./p2p-data \
  --platform-pubkey ea4a6c63e29c520abef5507b132ec5f9954776aebebe7b92421eea691446d22c \
  --audit-file ./repair-audit.jsonl
```

| 参数 | 说明（对应 crates/repair-helper/src/main.rs ServeArgs） |
|---|---|
| --bootstrap | rendezvous 地址，可重复；空则仅局域网可拨 |
| --data-dir | 身份数据目录（key.seed 持久化，重启身份不变），默认 ./p2p-data |
| --platform-pubkey | 平台 ed25519 公钥 hex 64 字符（票据验签），必填 |
| --audit-file | 审计 JSONL 落盘路径，启动截断，默认 ./repair-audit.jsonl |
| REPAIR_ROOTS | 授权根列表，环境变量。分隔符平台相关（T26 修复）：macOS/Linux 为 :，Windows 为 ;——与 split_roots 的 platform_separator() 一致；缺省为临时演示根 |

启动成功后 stderr 打印本机 PeerId：repair-helper p2p endpoint ready: peer <id>，
该值即铸票的 --helper-peer。Ctrl-C 优雅退出并焚毁票据台账
（repair-helper stopped; ticket ledger burned on exit）。

## 4. 服务侧：铸造票据 mint-ticket

票据把「助手 PeerId + 桥 PeerId + scope + 时效」绑成一次性凭证，helper 全项校验才受理：

```sh
repair-helper mint-ticket \
  --key key.seed \
  --helper-peer <HELPER_PEER> \
  --bridge-peer <BRIDGE_PEER> \
  --scope diag \
  --ttl 3600
```

| 参数 | 说明（对应 main.rs MintArgs） |
|---|---|
| --key | 平台签发密钥种子文件（32 字节），与 serve --platform-pubkey 同一密钥对 |
| --helper-peer | 助手 PeerId（serve stderr 输出） |
| --bridge-peer | 桥 PeerId，必须等于实际接入桥节点的身份 |
| --scope | diag（只读）或 fix（可写需审批） |
| --ttl | 有效期秒数 |

票据串 = base64url(canonical JSON payload) + "." + base64url(ed25519 签名)，
payload 字段 {ticket_id, helper_peer, bridge_peer, scope, iat, exp}（ticket.rs mint）。
mint 后立即自检 verify（同密钥公钥 + 同 bridge_peer 作入流对端），失败不产出；
票据走 stdout，自检信息走 stderr。

## 5. DSH mcpServers 接入模板

DSH 会话以 stdio MCP server 挂桥，mcpServers 指向 repair-bridge 命令行：

```json
{
  "mcpServers": {
    "repair": {
      "command": "repair-bridge",
      "args": [
        "--ticket", "/path/to/ticket",
        "--peer", "<HELPER_PEER>",
        "--bootstrap", "43.240.223.138/u3400"
      ]
    }
  }
}
```

- --ticket 两种来源（bridge load_ticket）：传入可读文件路径则读文件内容；
  否则按字面票据串使用。DSH 配置建议用文件路径（铸票 stdout 重定向落盘）。
- --peer 为助手 PeerId（bridge 侧显式指定，bridge 不校验票据，见 plan §3.3）。
- --bootstrap 必填且可重复（ArgAction::Append）。
- 桥启动流程（crates/repair-bridge/src/main.rs）：建临时身份节点（mdns 关）→
  等对端发现（30s 超时）→ 开 /repair/mcp/1 流 → 首帧发票据 → pump 对拷。
- 断线语义：p2p 流断或 stdin EOF → 排空在途帧后非零退出 + stderr 留因，不自动重连。

## 6. 端到端调用顺序（下单→授权→执行→交付→收货即焚）

1. 下单：客户描述问题，服务侧确定 scope（diag 免费体检 / fix 按单付费）。
2. 授权：客户机起 serve（§3），服务侧铸票（§4），票据一次性绑定双端身份与时效。
3. 执行：DSH 经 mcpServers 起桥，MCP initialize → tools/list → tools/call 驱动工具面；
   write/danger 调用在客户机挂起审批（P0b 为 60s 超时即拒，approval.rs APPROVAL_TIMEOUT）。
4. 交付：session_report 导出结构化执行记录（审计 JSONL 汇总），作为验收依据。
5. 收货即焚：确认收货 / 超时 / 断流后票据台账焚毁（ticket_ledger 一次性 claim），
   MCP server 停止——临时性是安全属性（design §4）。

## 7. 执法与审计语义速查

- 工具面 6 个（P0b 闭集）：sys_snapshot、fs_read、fs_list、fs_search、shell_exec、session_report。
- 执法顺序（repair-enforce Enforcer::evaluate）：红线（无条件拒，先于一切）→
  shell 白名单闭集 → 风险分级 → scope 门（diag 下 write/danger 直接拒）→
  fix 下 write/danger 进审批状态机（60s 超时 = 拒绝，不可配置放行）。
- shell_exec 参数：argv 数组 + 授权根内 cwd + timeout（缺省 60s 上限 300s）；
  白名单闭集数据 = 三类 playbook 命令并集（whitelist_data，T24）。
- 输出门禁：单结果 ≤ 256 KiB（cap.rs MAX_OUTPUT_BYTES），超限截断置 truncated。
- 审计：每次工具调用一行 JSON（时间戳/工具/参数摘要/风险档/审批结果/结果摘要/耗时），
  写失败必须留 error 日志（audit.rs，禁止静默丢弃）。
- MCP 方法集：initialize、notifications/initialized、ping、tools/list、tools/call；
  未知方法返回 -32601；版本协商取 SUPPORTED_VERSIONS 交集（lib.rs）。

## 8. Codex / Claude Code

与 DSH 同 stdio 桥原理同构：二者均为 MCP client，把 mcpServers 的 command/args
替换为同样的 repair-bridge 命令行（Codex 走 exec/proto、Claude Code 走 headless
stream-json，design §9），即可复用同一套工具面、票据与执法语义，无任何 crate 改动。
桥不解析 MCP 语义、协议升级不碰 Rust（design §6「三行配置即接入」）。

## 9. 已知限制与校准项

- 2026-09-04 勘误：serve/stdio 装配已接线 repair_enforce::builtin()（23 条 playbook
  命令并集），stdio/p2p 白名单语义一致；闭集外命令仍拒（not in closed whitelist）。
  同轮修复：session_report 补入 stdio 装配（六工具闭集）、盲拨连接 ping 应答、
  rendezvous 客户端链路失败判死重连（见 repair-p0b-drill.md §2a）。
- bridge 身份为进程临时（temp 目录含 pid，无 --data-dir 参数），重启身份即变；
  演练期需固定桥身份，方法见 repair-p0b-drill.md §1。
- 其余校准项（Remove-Item 红线冲突 / sys_snapshot Windows 分支 / REPAIR_ROOTS 分隔符 /
  fs_read 二进制 lossy）统一登记在 repair-p0b-drill.md §2。


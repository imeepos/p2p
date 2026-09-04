# P0b 真机演练清单（3 类问题诊断+修复各 1 例）

> 基线 main @ 4fe16ec（2026-09-03）| 对应 docs/design/remote-support-design.md §10 P0b 验收
> 「真机 3 类问题诊断+修复各 1 例」、docs/playbooks/ 三类草案。
> 定位：人工里程碑，不入账本；机械验收以 T20-T28 exit 0 为准（plan §1）。

## 1. 演练前置

两台机器角色：

| 机器 | 角色 | 运行内容 |
|---|---|---|
| 客户机 A | 被维修端点 | repair-helper serve（见 repair-runner-integration.md §3） |
| 服务侧 B | runner | DSH（或等价 MCP client）+ repair-bridge + mint-ticket |

材料与命令：

```sh
export PATH="$HOME/.cargo/bin:$PATH"
cargo build --release -p repair-bridge -p repair-helper

# 1) 平台测试密钥（与 itest 同源，seed = 32 字节 0x07）
#    平台公钥 hex：ea4a6c63e29c520abef5507b132ec5f9954776aebebe7b92421eea691446d22c
# 2) bootstrap 地址（二选一）：43.240.223.138/u3400（138）或 121.196.193.177/u3400（ECS）
# 3) 桥身份固定：bridge 无 --data-dir、身份目录 temp/repair-bridge-<pid> 每次新建，
#    用固定 seed 预置身份使重启后 PeerId 不变——wrapper 见下方「桥身份固定」。
```

桥身份固定 wrapper（T20 现状的操作补齐，无需改码）：

```sh
#!/bin/sh
# 以固定 seed 预置桥身份目录后 exec，保持同一 pid -> 同一身份
SEED=$HOME/p2p-lab/bridge.seed
D="$TMPDIR/repair-bridge-$$"
mkdir -p "$D" && cp "$SEED" "$D/key.seed"
exec repair-bridge "$@"
```

桥 PeerId 获取：先以任意票据起桥一次，helper 拒绝日志（warn "repair stream rejected ... peer=<B>"）
即桥真实 PeerId；以其铸票后复用同一 seed 起桥（身份不变），票据即可生效。
（bridge 不自打印自身 PeerId，属已知操作限制，登记 §2.5。）

票据铸造与启动顺序：

```sh
# A 机先起 serve（stderr 得到 HELPER_PEER）
# B 机铸票（BRIDGE_PEER 取自桥身份）
repair-helper mint-ticket --key key.seed \
  --helper-peer <HELPER_PEER> --bridge-peer <BRIDGE_PEER> \
  --scope diag --ttl 3600 > ticket
# DSH mcpServers 指向 repair-bridge --ticket <ticket文件> --peer <HELPER_PEER> --bootstrap <ADDR>
```

就绪检查：tools/list 返回 6 个工具名（sys_snapshot/fs_read/fs_list/fs_search/shell_exec/session_report）。

## 2. 已知校准项（如实登记，来自各单验收流转）

1. **Remove-Item danger 步被红线拦截（T24 流转，coordination 轮 65）**：
   c-drive 步骤 8 Remove-Item 命令（C:\Windows\SoftwareDistribution\Download\* -Recurse -Force）
   白名单命中（whitelist_data 有 Remove-Item 规则）但 redline BatchDelete 判定操作数 >= 2
   （-ErrorAction 的取值 SilentlyContinue 被 collect_operands 计入操作数）→ 无条件拒。
   真机校准点：playbook 命令形态或红线操作数判定，二选一收敛，回写 whitelist_data / 草案。
2. **sys_snapshot Windows 分支缺失（T23 遗留）**：快照复用 uname/df/sysctl（POSIX），
   Windows 下字段记 unavailable。P0b 真机演练优先 macOS/Linux；Windows 演练待补分支。
3. **REPAIR_ROOTS 分隔符平台差异（T26 已修，需演练验证）**：macOS/Linux 用 :、
   Windows 用 ;（jail.rs platform_separator）。演练在两平台各验证一次多根配置生效。
4. **fs_read 二进制 lossy（P1 登记）**：fs_read 以 from_utf8_lossy 解码（fs_read.rs），
   二进制文件内容有损；P0b 演练只读文本文件，二进制读取登记 P1 结构化输出。
5. **serve 装配空白名单 + 桥身份临时（T28 勘查补充）**：
   a) main.rs serve/stdio 注入 ShellWhitelist::empty()，CLI 演练 shell_exec 一律拒——
      白名单数据 builtin() 已就绪但未接线，修复类 shell 步骤本次演练预期观察拒绝路径；
   b) bridge 无 --data-dir、不自打印 PeerId，演练用 §1 wrapper 固定身份（P1 建议补参数）。

## 2a. 勘误与状态更新（2026-09-04 RS 排障轮，基线见 git log）

1. **2.5a 已失效：白名单已全形态接线**——main.rs shell_whitelist() 返回
   repair_enforce::builtin()（23 条 playbook 命令并集），stdio/p2p 两形态一致；
   闭集外命令仍拒（not in closed whitelist），修复类步骤按 fix scope 走审批。
2. **session_report 已接入 stdio 装配**——helper_registry 此前缺注册，tools/list
   实发 5 工具；现补齐为六工具闭集（registry_tests 锚定）。注意：工具调用为
   并发处理，session_report 导出的是处理时刻已完成的事件，runner 需在收尾
   （响应排空后）调用才能拿到全量对账。
3. **盲拨连接探活修复**——TransportLink 从不应答入站流，facade liveness probe
   3 次未达后约 33s 掐线，桥/演练连接频繁断；现挂 PingHandler 应答循环。
   回归锚定 crates/p2p-itest/tests/rendezvous_facade_link.rs（37s 探活窗口生存）。
4. **rendezvous 客户端判死重连**——链路级注册失败原先在死连接上 20s 空转、查询
   分支被 select 饿死；现失败即退避重连（±20% 抖动）+ 重连即重注册。
5. **票据文件需去尾随换行**——bridge load_ticket 按字面读文件，尾随 \n 会触发
   base64url Invalid symbol；铸票后用 tr -d '\n' 落盘。
6. **单机演练地址卫生约束**——helper 广播地址为 127.0.0.1 时，LAN/公网 rendezvous
   查询侧按 E5 卫生直接过滤（不可发现）；单机全链演练 bootstrap 须全 loopback
   地址（same_machine 语义保留可发现性），或让观测反射看到可路由地址。

## 3. 类别一：电脑卡慢（docs/playbooks/slow-diagnostics.md，performance-slow）

诊断（diag scope，只读）：

| 步骤 | playbook 步骤 | 工具/命令 | 预期与观测 |
|---|---|---|---|
| 1 | 采集系统快照 | sys_snapshot | 返回 os/arch/CPU/内存/磁盘字段，无错误 |
| 2 | 高 CPU 进程 | shell_exec Get-Process 排序 | 复合命令闭集外 → 拒绝（观测原因） |
| 3 | 高内存进程 | shell_exec Get-Process 排序 | 同上，拒绝路径留审计 |
| 4 | 磁盘剩余 | shell_exec Get-CimInstance | 复合命令闭集外 → 拒绝 |
| 5 | 内存容量 | shell_exec Get-CimInstance | 同上 |
| 8 | 复查快照 | sys_snapshot | 修复后指标不高于修复前 |

修复（fix scope；P0b 装配空白名单下预期全拒，记录拒绝原因即达标；接线 builtin() 后复跑）：

| 步骤 | playbook 步骤 | 命令 | 预期 |
|---|---|---|---|
| 6 | 结束确认进程 | Stop-Process -Id <PID> -Force | 白名单命中 → 审批（60s 超时拒）或放行执行 |
| 7 | 清理用户临时文件 | Get-ChildItem $env:TEMP ... Remove-Item | 复合命令闭集外 → 拒绝 |

## 4. 类别二：弹窗流氓软件（docs/playbooks/popup-malware-cleanup.md，popup-adware）

诊断（diag）：

| 步骤 | playbook 步骤 | 工具/命令 | 预期与观测 |
|---|---|---|---|
| 1 | 系统快照 | sys_snapshot | 基线无错误 |
| 2 | 带窗口进程 | shell_exec Get-Process MainWindowTitle | 复合命令闭集外 → 拒绝 |
| 3 | 已安装程序 | shell_exec Get-ItemProperty Uninstall | 复合命令闭集外 → 拒绝 |
| 4 | 启动项 | shell_exec Win32_StartupCommand | 复合命令闭集外 → 拒绝 |
| 5 | 计划任务 | shell_exec Get-ScheduledTask | 复合命令闭集外 → 拒绝 |
| 9 | 系统代理 | shell_exec netsh winhttp show proxy | 白名单命中（接线 builtin() 后 read 放行；当前空白名单装配下拒） |

修复（fix）：Stop-Process -Name 白名单命中需审批；Get-Package | Uninstall-Package 复合命令闭集外拒；
Disable-ScheduledTask 白名单命中需审批；netsh winhttp reset proxy 白名单命中需审批。
P0b 装配下记录拒绝/超时路径，接线后复跑放行路径。

## 5. 类别三：C 盘空间（docs/playbooks/c-drive-space-cleanup.md，disk-space-low）

诊断（diag）：

| 步骤 | playbook 步骤 | 工具/命令 | 预期与观测 |
|---|---|---|---|
| 1 | 磁盘剩余 | shell_exec Get-CimInstance | 复合命令闭集外 → 拒绝 |
| 2 | %TEMP% 占用 | shell_exec Get-ChildItem Measure | 复合命令闭集外 → 拒绝 |
| 3 | 回收站占用 | shell_exec (New-Object ...) | 复合命令闭集外 → 拒绝 |
| 4 | 更新缓存占用 | shell_exec Get-ChildItem SoftwareDistribution | 复合命令闭集外 → 拒绝 |

修复（fix）：Clear-RecycleBin / Stop-Service wuauserv / Start-Service wuauserv / Dism.exe 白名单命中
→ 审批；Remove-Item 更新缓存步命中红线（校准项 1）；Stop-Service 后恢复为强制步骤（playbook 步骤 9）。

## 6. 观测点

- **日志**：helper stderr（默认 info，RUST_LOG 可调）：受理/拒绝行含 ticket_id/scope/peer；
  bridge stderr：断线原因、非零退出码。
- **审计 JSONL**：--audit-file 每调用一行（camelCase）：时间戳/工具/参数摘要/风险档/
  审批结果/结果摘要/耗时；拒绝与放行均留痕；写失败必留 error 日志。
- **session_report 导出**：tools/call session_report 返回 JSON（ticketId + count + 调用记录），
  与审计 JSONL 逐条可对账；验收记录以此为准。

## 7. 回滚与急停

- P0b 工具面无 fs_write/fs_edit（只读四件套 + shell 白名单），无文件修改，回滚面 = 无；
  shell 步骤在 diag 档直接被拒、fix 档审批超时即拒，均不 spawn。
- 急停：客户机 Ctrl-C 停 serve（台账焚毁、MCP server 停止）；服务侧 kill 桥进程即可断流。
- 演练中任何一步异常：先留日志/审计证据，再按「类别」重跑该步骤，禁止静默跳过。

## 8. 验收记录表模板

| 项 | 内容 |
|---|---|
| 类别 / playbook | performance-slow / popup-adware / disk-space-low |
| 日期 / 基线 | yyyy-mm-dd / main @ 4fe16ec |
| 客户机 OS / 架构 | macOS 15 arm64（示例） |
| HELPER_PEER / BRIDGE_PEER | base58 各记前缀 |
| scope / ttl | diag / fix + 秒数 |
| 诊断步骤通过 | 步骤号列表（只读工具 + 白名单放行） |
| 拒绝路径记录 | 工具名 + 拒绝原因（红线/闭集外/scope/超时）+ 审计行号 |
| 修复步骤 | 放行执行结果 / 审批拒绝 / 超时拒绝 |
| session_report 对账 | ticketId、count 与 JSONL 行数一致 |
| 校准项回写 | 命中项（§2.1-2.5）实际行为 + 建议 |
| 遗留问题 | 进入协调裁决/下一轮 |


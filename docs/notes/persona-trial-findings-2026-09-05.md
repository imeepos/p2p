# 多岗位角色 CLI 试用 findings（2026-09-05）

试运行者：协调会话扮演 5 个岗位角色，仅用 CLI 与 docs/ops 文档，未读源码。
隔离目录 /tmp/persona-trial/{a,b,sre,ai,it,net,net2}；p2pctl=main@f864159 全新构建。
前置：docs/notes/ai-pilot-findings.md（2026-09-04 AI 盲跑轮）8 条摩擦已回填，本轮不重复记录已修项。

## 结论速览

- 聊天域对真人角色存在结构性缺陷：**端口漂移 + 好友地址不可修 = 身份一次性**（F1，致命）；**serve 与 send 同身份互斥 = 无法边收边发**（F2，高）。
- 观测/幂等/报错文案是强项：node 域结构化 status、幂等 stop、deny 不存在条的「默认拒绝」提示、PeerId 校验带长度诊断，均获角色好评。
- 发现 1 个机械缺陷：node start --json 重定向文件时输出丢失（F9）。

## 角色与场景覆盖

| 角色 | 岗位视角 | 场景 | 结果 |
|---|---|---|---|
| 林晓×麦总 | 自由设计师↔客户 | 加好友/邀请/收发/附件/离线投递/群聊 | 互通成功但不可持续（F1/F2/F3） |
| 周正 | SRE 运维 | 节点生命周期/指标/日志/幂等 | 基本盘好；日志域错位（F7）、公网默认静默（F8） |
| 陈默 | AI 应用开发者 | llm-share 借出全链 | 走通；身份初始化无正门（F6）、借方入口缺位（F11） |
| 凯哥 | IT 远程支持 | acp allow/deny/list 全路径 | 报错文案优；无法发现对端 PeerId（F10） |
| 大刘 | 网管 | peer dial/ping/disconnect/诊断 | 拨号通；无 peer list/发现/中继只读查询（F10） |

## 摩擦清单

### F1（致命）chat 身份一次性：重启即永久失联
- 症状：麦总重启 chat serve 后监听端口漂移（u56625→u59173），林晓好友簿仍存旧地址，后续消息全部 status=pending。
- 三因复合：serve 端口默认随机；chat friends update 明文「addrs 不可经此修改」无任何修复途径；serve 默认关 mDNS 且无 rendezvous 参数。
- 用户出路只剩 remove+re-add（对方还得重发邀请），IM 断联不可自愈。
- 建议：serve 端口记忆（首启随机后写配置，重启沿用）；chat 层成功会话自动学习回写对端 addrs（底座 AddrCache 能力已有）；friends update 开放 --addr。

### F2（高）同身份无法边收边发
- 症状：收消息必须 chat serve 常驻（持 identity.lock），同目录 chat send 立即退出 1。serve 无任何发送通道（--help 无交互/委托入口）。
- 用户模型：人类用户一个身份要同时收发；现拓扑要求收发分身（两个数据目录），或半双工轮流起停。
- 建议：serve 增加本地控制通道（仿 daemon.sock 先例），chat send 检测 serve 存活时委托发送；次选 serve stdin 协议。需设计裁决，独立立卡（PR5）。

### F3（中）离线投递语义与闭环断裂
- 症状：对端离线 send 返回 status=pending 却 exit=1（「已存待投」按运行失败处理，自动化误判）；对端恢复后 pending 无自动重投（受 F1 复合影响），且无 chat outbox list/flush 观测与手动补偿命令。
- 建议：pending 语义改 exit 0（delivered 与否看 status 字段）；serve 启动/周期 flush 行箱；补 outbox 子命令域。

### F4（中）invites 写命令拒绝 --json
- 症状：chat friends invites accept <PEER> --json 报 exit=2 unexpected argument；§1.2 全局约定「写命令同约定」双形态。reject/cancel 同族待查。
- 建议：补齐 --json。

### F5（中）accept 后对端显示名退化与语义错位
- 症状：B accept 后好友簿 nickname=对端 PeerId 原文；A 簿中 B 的名字显示为「B 给 A 起的备注名」——邀请 nickname 是邀请方对受邀方的备注，被 accept 方错当对端自称沿用。
- 建议：邀请帧加发起方自称字段（契约加法）；缺省回退 PeerId 缩略（update --nickname 空串已有缩略机制，口径对齐）。

### F6（中）身份初始化无正门
- 症状：identity 域仅 reset；新目录首命令必败（offer publish 报 key.seed 不存在），只能借道 node start/chat serve 造身份；无 identity show 类只读查询（AI 轮 F5 遗留，chat/node 双身份仍只能起进程看首行）。
- 建议：identity init（显式创建，0600，--json 输出 peerId）与 identity show（--domain node|chat）。

### F7（中）日志域错位：SRE 找不到节点日志命令
- 症状：log 域读 GUI frontend.log 且参数 --log-dir（他域均 --data-dir）；daemon.log 无对应命令（status 有 logPath 指路，算半个），SRE 只能裸 tail 文件。
- 建议：node log tail 子命令；log 域兼容 --data-dir 别名或文档显式区分三路日志。

### F8（中）公网默认静默外联
- 症状：node start 默认连公网 bootstrap/relay/observation（relaySessionsActive=2、daemon.log 可见 43.240.223.138/121.196.193.177）；config get 可见默认值但启动输出无任何「正在连接公共设施」声明；无 lan-only 开关。
- 建议：启动输出声明公网端点；config/CLI 提供 lan-only 模式；文档安全章注明元数据经公共节点。

### F9（高，机械缺陷）node start --json 重定向丢输出
- 症状：node start --json > file 文件 0 字节且 exit=0，daemon 实际已启动（status 证）；前台/管道可见。疑似 fork 前 stdio 缓冲未 flush（双缓冲丢写）。
- 影响：脚本/自动化拿不到 peerId/addr（T23 类冒烟、CI、自启服务全踩）。
- 建议：启动结果经 daemon.meta.json 落盘后由父进程读回渲染；或 fork 前 flush+子进程关 fds。机械回归：重定向文件断言非空且 JSON 可解析。

### F10（中）CLI 无发现/对端清单/中继只读查询
- 症状：peer 域无 list（地址簿与在线态不可见）；p2pctl 无 discovery/relay 域（GUI 有页，服务器纯 CLI 环境无替代）；凯哥之问「想连我的人我怎么知道他 PeerId」无 CLI 答案。
- 建议：peer list（簿+在线态）、discovery list（邻居缓存）、relay status（会话/水位）三个只读命令，与 GUI 对等补齐。

### F11（P2/Phase 1）llm-share 借方调用入口缺位
- 症状：CLI 只有出借方管理面与双边流水；借方无 borrow/use 命令（T23 冒烟由生成式 harness 承载，产品面缺位）。Phase 0 边界如此，需求拉动后开卡（PR6 登记不实施）。

### 小项
- F12 附件 mime 恒 application/octet-stream（.txt 也是），建议按扩展名/内容嗅探。
- F13 peer ping rtt_ms=0 亚毫秒精度丢失（建议 0.1ms 精度或 <1ms 标注）；dial 输出 hops=[] 语义无解释。
- F14 headless-metrics.mjs 把 --help 当未知参数 die(4)（OPS1 验收命令含 --help 检查，靠分号逃过）；建议补 usage 输出 exit 0。
- F15 cargo build p2pctl 有 dead_code 警告，清理（clippy 门禁未覆盖 bin 的该路径）。
- 环境观察：检出 /Users/imeepos/ext512/p2p-invite 存在 6 个泄漏 chat serve 进程（非本仓产物，未处置）；建议进程巡检覆盖同机多 checkout（patrol-cron 项）。

## 好评清单（保持不退步）
- node status 未启动结构化 running=false+reason；stop 二次幂等；start 防呆 alreadyRunning。
- acp deny 不存在条目：「本就默认拒绝，无需 deny」——把安全语义讲给用户听。
- PeerId 非法报错带实际解码长度；scope 枚举列出 possible values。
- group create 帮助把约束（成员⊆好友簿、≤32、群名 1..=64）写在 usage 里。

## 需求映射（立卡见 .devloop/loop-state.json）

| 卡 | 内容 | 优先级 | 覆盖 |
|---|---|---|---|
| PR1 | chat 可达性与行箱闭环（端口记忆/地址学习/update --addr/outbox 域/pending 退出码/invites --json/昵称与 mime） | P0 | F1 F3 F4 F5 F12 |
| PR2 | CLI 观测对等 + node start 输出修复（peer list/discovery list/relay status/node log tail/--data-dir 兼容/重定向回归） | P1 | F7 F9 F10 F13 |
| PR3 | 身份正门与一致性小包（identity init/show、receipt verify 本机公钥缺省、headless-metrics --help、dead_code 清理） | P1 | F6 F14 F15 |
| PR4 | 公网默认显式化与 lan-only | P2 | F8 |
| PR5 | chat 双工控制通道（需设计裁决，PR1 合并后） | P1 | F2 |
| PR6 | llm-share 借方调用入口（Phase 1 决策门，需求登记不实施） | P2 | F11 |

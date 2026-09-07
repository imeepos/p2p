# p2p GUI 真实流程终验报告（UX-R2 Realflow Verify，2026-09-07）

终验人：p2p GUI 真实流程终验员（AI，UX-R2 真流程终验）。仅验证、仅产出本报告与 .agents 经验回填，未改任何业务代码。

## 0. 基线与方法

- 基线：main @ d54c6bd（R2F-A/B/C 三卡全合入）；终验构建时 main tip = 2ba0f2d（= d54c6bd + share-join 合入 + skill docs，R2 修复全部在位）。桌面二进制（p2p-console / p2pctl / acp-console / acp-agent）于终验前从该 tip 全量重建（pnpm build + cargo build，BUILD-ALL-OK），前端 dist 为真实 IPC 态（生产构建无 VITE_MOCK_IPC）。
- 真实拓扑（A/C 项）：
  - 本机 bootstrap（llm-share-smoke 同款 harness）：192.168.0.15/u35510（rendezvous + 观测反射 35512）；
  - 出借方：102 服务器 192.168.0.102/u35520，真实节点身份 51SBwggq…6ge3，经产品 CLI `p2pctl llm-share offer publish` 签名发布 realflow-model（TTL 至 2026-12-31）；上游为进程内 mock（T23 合规边界内，网络面/账本面/签名面全真实）；
  - 借入方：桌面 GUI 实例，HOME 隔离至临时目录，节点身份独立铸造 7RixQwks…p2pdB（出借方白名单按此 PeerId 精确放行）。
- GUI 驱动方式（桌面 WKWebView 无 CDP）：GC1 控制通道（p2pctl gui status/navigate/page/screenshot/invoke）+ 自制 Swift AX 驱动按 pid 直连 AXUIElement（dump/find/press/focus/键入/剪贴板粘贴）；键入用逐字符 CGEvent 真实按键（React 受控组件 onChange 真实触发），JSON 用剪贴板 Cmd+V 粘贴规避布局差异。
- 双机冒烟：bash scripts/ops/llm-share-smoke.sh 全绿（SMOKE-OK，S1–S6 全 PASS），关键摘录：
  - CALL-OK req_id=req-t23-s4 sse_frames=3 usage=21/9 net=-30 + RECEIPT-VERIFY=PASS LEDGER-APPLY=true（产品 CLI 验签）
  - REJECT-OK code=NotAllowlisted msg=peer H45RJth… not in allowlist，上游调用计数增量为零
  - STREAM-BROKEN estimated=true usage=47/14 + estimated 收据验签 PASS

## 1. 结论速览

| # | 项 | 结论 |
|---|---|---|
| A-1 | llm-share 真实双机业务闭环（白名单→借用→双边账本→收据验签） | 实测通过 |
| A-2 | R2-01 借用完成后账本/净差联动刷新（真实数据） | 实测通过 |
| A-3 | R2-07⑤ 拒绝路径人话原因（真实 NotAllowlisted） | 实测通过 |
| B | R2-24 ACP 权限请求自然路径（真机） | 环境受限 |
| C-1 | R2-16 事件详情折叠+复制（真实事件） | 实测通过 |
| C-2 | R2-17 概览/事件页 PeerId 缩略 6+4 同口径（真实节点） | 实测通过 |
| D-1 | R2-20 文档内链接点击反馈（桌面真实点击） | 实测通过（外链无实链，见 5.1） |
| D-2 | R2-21 返回顶部（浏览器态复核） | 环境受限（附替代证据） |
| D-3 | R2-22 取消跳过（浏览器态复核） | 环境受限（附替代证据） |
| RF-1 | 新发现：净差明细行数字缺失（真实 IPC 视图缺字段） | 实测不通过 |
| RF-2 | 新发现：borrow「消息」文案承诺纯文本、真实后端要求 JSON 数组且内部错误直出 | 实测不通过 |

计数：**通过 6 / 不通过 2 / 受限 3**。

## 2. A：llm-share 真实业务闭环（最高优先）

### 2.1 A-1 真实双机闭环：实测通过

流程全记录：102 侧身份+签名发布 → serve（--allow 7RixQwks…）→ GUI 真实节点经 rendezvous 发现出借方（概览实时出现「发现节点 51SBwg…6ge3（192.168.0.102/u35520）」）→ 在 GUI borrow 表单真实填写并提交（出借方 PeerId / 模型 realflow-model / maxTokens 64 / messages 为 JSON 数组）→ 二次确认弹窗（明示「模型 realflow-model，maxTokens 上限 64，出借方 51SBwggqDk9MbhKq4jSpJiAuNNaypreCJe9pAkXf6ge3」）→ 跨机真实代理调用 → GUI 报告卡（AX 原文）：

```text
借用完成 | 请求编号 dea45205-d211-4520-92a8-94b49d0e0975 | 入账状态 已入账
SSE 事件数 3 | 用量（入/出）21/9 | 争议窗口 24 小时
```

双边对账（借入方本地 DATA/llm-share/ledger.json，含 Ed25519 签名全文）：

```text
{"v":1,"req_id":"dea45205-d211-4520-92a8-94b49d0e0975","period":"2026-09",
 "lender":"51SBwggqDk9MbhKq4jSpJiAuNNaypreCJe9pAkXf6ge3","borrower":"7RixQwks6hTRcSbtC19VsTP78zFcQXeNqTGf2r8Cp2dB",
 "model":"realflow-model","usage":{"input":21,"output":9},"estimated":false,
 "sig":"2xMtbFPExDS2Yqbd4g1G6g7qF7rCX1hBojYKfxVTqTGSPoTNZvu6gytoYdETKY45trkWA8aczAqRo6jAxdebi61h"}
```

- 出借方 102 侧：upstream.jsonl 行数 1（上游恰一次调用）、serve 日志 receipts=1（出借账本恰一笔）；
- 借入方侧产品 CLI 复核：p2pctl llm-share ledger list → 「共 1 条流水 … tokens=30 (in=21 out=9) estimated=false」，与 GUI 报告卡逐字段一致；
- 另：双机冒烟 S2–S6（发现/验签选路/收据验签/负向拒绝/断流估算收据）同轮全绿。

### 2.2 A-2 R2-01 联动刷新（真实数据）：实测通过

- 借用确认后未做任何手动刷新，18s 后 AX 抓取：净差卡由「本机未参与任何流水」变为真实行「借入 | 51SBwg…6ge3 | 2026-09 | -30」；流水明细出现真实行且统计行为「共 1 条 · 合计 30 tokens」（R2-10 统计行一并真实复现）；
- 拒绝路径（2.3）后净差/流水无变化（不误刷）；
- 页面重挂载（导航离开再回）后净差/流水仍正确呈现真实账本（GUI 读侧直读同一磁盘事实源）。

### 2.3 A-3 R2-07⑤ 拒绝路径人话原因：实测通过

出借方重启为默认拒绝（空白名单）后 GUI 再次借用，真实结构化拒绝到达，报告卡 AX 原文：

```text
已被拒绝
原因: 本机不在该出借方的白名单内：请对方将本机 PeerId 加入其白名单后重试
拒绝码: not_allowlisted   [复制详情]
入账状态: 未入账（请求编号复用 dea45205-…，重试幂等语义保持）
```

点击「复制详情」后剪贴板实文（pbpaste）：「本机不在该出借方的白名单内：请对方将本机 PeerId 加入其白名单后重试」+ 换行 + 「code: not_allowlisted」——人话原因+可操作出路+拒绝码保留+复制详情四要素齐备。

## 3. B：R2-24 ACP 权限请求自然路径：环境受限

受限事实：自然路径终点是「真实 dsh 子进程发出 request_permission（execute 类）→ 桥 ask 路由 remote_gui → chat agent 形态指示条」。本机 ~/.dsh/profiles/ 现有 profile 为 default/web/headless/real-base 等，无 acp profile（dsh 0.1.1-rc.2，--profile <name> 按 $DSH_HOME/profiles/<name> 装载），`dsh --profile acp` 无法启动真实 agent 子进程；用 acp-echo-stub 顶替属造数造 agent，不构成「真实权限请求」，不做。

本轮已真实验达的前置链路（如实记录，非终验通过项）：桌面 GUI 真实托管 acp-console 伴生进程（日志「acp-console 定位成功…子进程已启动」「acp-console ready ws=127.0.0.1:64865 status=127.0.0.1:64866」，mDNS 发现面开启）；GC1 acp 页动作面（connect/newSession/sendPrompt）真实可用；桥权限瀑布约定（read/think/fetch 自动放行、execute 走 remote_gui）经 apps/acp-agent/README.md 复核在位。

最小复现条件（真机可测路径）：① 机器安装含 acp profile 的 dsh（pnpm dsh --profile acp 可独立起 ACP 子进程）；② acp-agent 启动并以 `p2pctl acp allow <consolePeerId> --scope sandbox --ask-route remote_gui` 放行 console 节点（TOFU 从 conn-denied 审计取 PeerId）；③ GUI（托管 acp-console）连上 agent、新建会话并发送会触发 execute 类工具调用的 prompt；④ 桥把 request_permission 透传至 GUI 后，验证 chat agent 形态 warning 指示条（「有 1 条待应答权限请求」）+「去处理」深链直开通讯录 Agent 权限面板。

## 4. C：事件流真实数据：实测通过

- 真实事件源：GUI 真实节点（mDNS 开启）对局域网/rendezvous 的真实发现，以及 GUI「手动拨号」表单真实拨通 102 出借方（拨号结果「拨号成功」）产生的真实连接事件；GC1 page 事件页 state 抓到真实事件体（peer_discovered / peer_connected，含 44 位完整 PeerId 与多地址数组，source=mdns/rendezvous）；
- R2-16 实测通过：事件行「详情」展开后为「接收时间 2026/9/6 21:21:23」+「原始负载」折叠态——JSON 默认不可见，「展开 JSON」切换与「复制详情」按钮在场；点「复制详情」后 pbpaste 为完整可解析 JSON：type=peer_discovered，peer=51SBwggq…（完整 44 位），addrs[0]=192.168.0.102/u35520，source=rendezvous；
- R2-17 实测通过：同一真实节点两处口径一致——概览最近事件「发现节点 51SBwg…6ge3（192.168.0.102/u35520）」「节点已连接 51SBwg…6ge3」；事件页行「21:23:36 连接 节点已连接 51SBwg…6ge3」「21:21:23 发现 发现节点 51SBwg…6ge3（…）」——均为前 6…后 4；事件页域分组筛选真实命中（「连接，1 条」）。

## 5. D：文档与更新卡

### 5.1 R2-20 文档内链接（桌面真实点击）：实测通过

- 跨文链接渲染人话标题（「最小接入路径（quickstart）」等，AX 链接清单无裸仓库路径）；点击后应用内切篇：正文 H1 由「p2p-base 通信协议接入文档」变为「最小接入路径（quickstart）」，左侧文档目录同步；
- 不可达仓库路径链接（渲染短名 wire-protocol，title 保留原路径）真实点击后：toast 两行实文「该链接暂不支持在应用内打开」+「文档路径已复制：../design/wire-protocol.md」，剪贴板 pbpaste = ../design/wire-protocol.md；
- 外链行为：现行五篇文档零外链（AX 链接清单仅内跨文与 blocked 两类），桌面系统浏览器路径无自然可测实链——如实标注为「无实链可测」，不作为通过/不通过对象。

### 5.2 R2-21 返回顶部 / R2-22 取消跳过（浏览器态复核）：环境受限（附替代证据）

受限事实：本会话沙箱内 node/pnpm 子进程加载内建模块挂起（require node:child_process 不返回；vitest/pnpm 后期同样挂起），Chrome --remote-debugging-pipe 在本沙箱子进程即退无法建链，自研 python CDP-pipe 驱动亦超时；故浏览器态页面复核通道不可用。桌面真实形态下 R2-22 亦天然不可达：设置「关于与更新」卡实测处于「检查失败」态（无真实更新源），无「跳过此版本」可点。

替代证据（如实标注层级）：基线门禁全绿含 docs-view.test.tsx（返回顶部：默认隐藏→滚出 240px 显→点击回顶即隐，testid docs-back-top，aria-label 走 i18n）与 about-update-card.test.tsx R2-22 专项用例（跳过→行+localStorage 持久化→取消跳过→行消失+键清空）；交付报告 R2F-C 含 mock 页面实测（5179 实例，skipped-version localStorage 断言）。本轮未能在浏览器/桌面重新目击，不据此改判。

## 6. 新发现（真实数据下暴露，mock 态被掩盖）

| # | 页面 | 现象（实测证据） | 初判根因 | 优先级 |
|---|---|---|---|---|
| RF-1 | /llm-share·净差 | 真实借用入账后净差明细行渲染为「出借 · 借入 · 笔」——三个数字全空（同屏统计行「共 1 条 · 合计 30 tokens」数字正常，排除渲染器问题） | views.rs LlmBalanceGroup 仅含 lender/period/netAmount/direction（camelCase serde），无 lentOut/borrowed/entries；前端 ledger-balance.tsx 却以这三键插值——mock 造数带全字段故交付走查未暴露 | P2 |
| RF-2 | /llm-share·借用表单 | 「消息」占位与提示承诺纯文本（「输入要发送给模型的消息…」「以文本形式发送给出借方模型的消息内容」）；真实提交纯文本 realflow plain text probe 得到内部错误直出：--messages 不是合法 JSON: expected value at line 1 column 1 | 前端把纯文本原文塞 messages_json，后端 parse_messages 仅收 OpenAI 数组 JSON；文案、校验与后端契约三方不一致（R2-07 同族「内部错误直出」回潮） | P1 |

## 7. 环境备注与收尾

- 走查/验证端口：bootstrap 35510/35512、出借方 102:35520、GUI 配置端口 35530/35531、vite dev 5183（mock 复核尝试用，已停）；shell 显式注入所需 env；全程未触碰并行会话实例（含其在跑的 GUI 进程）。
- 收尾已执行：本机 p2p-console/acp-console/bootstrap/vite 全部终止；102 出借方进程全杀，~/llm-smoke-work 下 lender-rf、logs-rf、run-rf、lender-restart.sh 已删（保留 src/harness/boot 与冒烟共享缓存，同冒烟脚本自身清理惯例）；本地造数目录 /tmp/uxr2rf*（含隔离 HOME、截图、AX dump、日志）已清理。
- 证据文本摘录已全部内嵌上文（AX dump / CLI 输出 / pbpaste / ledger.json 全文），不依赖临时文件。
- 附注：真机 AX 驱动（Swift 按 pid 直连 AXUIElement）与 GC1 页面语义协议组合可远程驱动桌面 GUI 完成真实点击/表单/读数闭环，建议纳入后续真机验收工具链；本轮按纪律脚本未入库（仅 docs 与 .agents 提交）。


## 8. 附录（同阶段收口记录，2026-09-07 补）

- **RF-1/RF-2 已修复**（commit 2c541c2）：borrow「消息」在 IPC 适配层兼容纯文本（messages_payload：数组 JSON 透传、其余包装为单条用户消息，对齐 CLI --prompt 语义），内部错误直出路径消除；LlmBalanceGroup 补 lentOut/borrowed/entries 且 direction wire 值对齐前端 lent/borrowed/flat，新增消费方键集断言防契约再漂移；gui-contract §16.1 表格同步。src-tauri 全量 120 用例绿。
- **D-2/D-3（R2-21/22）浏览器态复核通过**（专属复核会话执行，17/17 断言 PASS）：R2-21 quickstart 篇滚动后返回顶部按钮出现、点击回顶即隐；R2-22 跳过→「已跳过版本」行+update-unskip+localStorage 记录在，取消后三者全清。证据：/tmp/verify-r22122/（assertions.json + 4 张截图）。原「受限」两项据此升级为**实测通过（浏览器态）**。
- **验收工具链补齐**（commit dc7caec，会话 B 交付、主会话验收合并）：GC1 控制面路由白名单与前端页面注册表接入 llm-share（只读观测页），p2pctl 帮助文本同步，navigate/health/注册表 describe 测试齐备——真机验收对 llm-share 页的远程盲区消除。门禁：lint/typecheck/vitest 1148/build + cargo build + control_channel 11 全绿。
- 本轮最终计数（含附录）：**通过 8 / 不通过 0 / 受限 1（仅 R2-24 待 dsh acp profile 环境）**。

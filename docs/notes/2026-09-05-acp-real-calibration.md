# ACP 真机对拍记录（ACP-CAL-1，2026-09-05）

目的：GUI 的 ACP 客户端薄层（ACP6a）此前仅对 acp-console README 契约自写
mock 自验，存在同源风险。本卡以真机链路逐项对拍，偏差以实物为准修 GUI 侧。

## 环境与启动

- commit：main == origin/main == 544b1ae（对拍与修复同基线）。
- 构建：`cargo build`（apps/acp-console，独立 workspace）、`cargo build --bins`
  （apps/acp-agent）、Node v24.20.0（原生 WebSocket，与浏览器事件面同型）。
- console 启动（stdout 就绪行给出 ws/status 端口与 token）：

  `acp-console --data-dir /tmp/acp-cal/console-data --ws-port 0 --status-port 0 --peer <agentPeer>@127.0.0.1/u47101`

- agent 侧（R2）：真实 acp-agent 桥 + acp-echo-stub 首选，但两者互连在握手层
  失败（见"遗留风险 1"），按卡面降级用 acp-console 回环测试设施
  （tests/common/mod.rs AgentMock：acp-common 裸 ndjson 握手应答 + 字节 echo）
  进程化为 /tmp/cal-agent（1:1 复刻，中立透传，不迎合 GUI mock 假设）。
- 驱动：Node 原生 WebSocket 探针（事件序列、关断码、帧字节级 dump）。
- 帧证据样例（回环 echo，session/list 等 3 行并入 1 帧；200KB 单行拆成约 50
  个 4KB 帧；上行请求帧尾不带换行时 acp-agent 侧行重组器不会输出）。

## 对拍矩阵（9 项）

| # | 项 | mock 假设（改前） | 实测 | 结论 / 修复指向 |
|---|---|---|---|---|
| a | WS 鉴权与错 token 关断码 | 错/缺 token -> Close(4403, "denied:bad-token") | HTTP 升级层 401 拒绝（console 日志 "401 Unauthorized bad/missing token"），客户端只见 error + Close(1006, 空 reason) | 偏差 D1：GUI 永远收不到 4403 表 token 错。修 mock-acp-ws 对齐 401 形；GUI 对 1006 呈 abnormal（view 测试同步） |
| b | 未知 peer 关断码 | 不经 onopen 直接 Close(4500, "dial-failed") | 先 onopen（console accept 后拨号），失败才 Close(4500)；地址不可达 peer 拨号 ~10s 超时后同样 4500 | 偏差 D2：关断时序。修 mock 先 open 后 4500；GUI 侧 4500 语义本就正确 |
| c | initialize 形状 | {protocolVersion:1, clientCapabilities:{fs}} -> {protocolVersion, agentInfo, agentCapabilities{loadSession, promptCapabilities}} | 传输面：echo 逐字节保真；语义面：与 ACP v1 官方 spec 一致（result 另有 authMethods 等增量字段，GUI 未建模不破坏） | 与 mock 一致（spec 佐证），无修复 |
| d | session/new 形状 | 发 {cwd:null}，回 {sessionId} | 传输面保真；spec 要求 params.cwd 为必填字符串（cwd:null 严格 agent 会拒） | 偏差 D3（记录不改码）：GUI 无 cwd 来源，等 ACP4 桥 cwd 改写落地再补；列遗留风险 3 |
| e | session/update 流 | params{sessionId, update{sessionUpdate, content}}，一块一帧 | 字段名/嵌套与 spec 一致（sessionUpdate 判别字段 + content block；messageId 可选 GUI 忽略）；分块粒度：块级增量，传输层一帧可含多块、一块可跨帧 | 字段与 mock 一致；传输粒度偏差 D4 -> 修 acp-connection 行重组（见 i） |
| f | session/cancel 与 stopReason | notification{sessionId}；cancel 后 prompt 结算 "cancelled" | spec：cancel 为 notification{sessionId}，prompt 响应 stopReason 枚举 end_turn/max_tokens/max_turn_requests/refusal/cancelled；mock 取值合法；cancel 经链路透传实测（echo 面字节保真） | 与 mock 一致（spec 佐证） |
| g | session/list / resume / close | list 回 {sessions:[...]}；resume/close 发 {sessionId}；错误 -32002 "session not found" | list、resume 均为 ACP v1 真实方法（分别挂 sessionCapabilities.list / .resume 能力位）；**close 不是契约方法，v1 为 session/delete**（sessionCapabilities.delete） | 偏差 D5：GUI 上行改 session/delete（acp-connection.sessionDelete），mock/测试同步 |
| h | 对端断流 GUI 看到的关断 | dropAll -> Close(1000, "agent-stream-dropped") | agent 进程死亡：console swarm 探针 ~30s 后判死（日志 "closing unresponsive connection after probe misses"），泵结束（end=Failed）但**不发任何 Close 帧**，客户端见 1006 空 reason；仅优雅 EOF 路径发 Close(1000, "peer closed")；另外客户端先 close(1000) 时 console 不回 Close 帧，客户端同样见 1006 | 偏差 D6：mock dropAll 默认 1006 空 reason；GUI closeInfo 如实呈 1006/abnormal，重连语义不变 |
| i | 帧格式 | 一帧 = 一行 JSON 文本（Text） | **Binary 帧**（binaryType 默认 blob，String() 直接毁帧）；一帧可含多行（64KiB 块合帧）；一行可跨多帧（200KB 行拆 ~50 个 4KB 帧）；无心跳帧、无 batch 帧，行界仅 "\n"；上行不带行尾换行会在 agent 侧行重组器处挂起 | 偏差 D7（最重）：GUI 修 acp-connection（binaryType=arraybuffer + decodeFrame + NdjsonAssembler 行重组）+ 上行帧尾补 "\n"；mock 下行改二进制帧并新增合帧入口 |

## 偏差修复清单（全部在 apps/gui/src/acp/**，各带回归测试）

- D1/D2/D6：mock-acp-ws.ts（401 形 1006、先 open 后 4500、deniedPeers 4403、
  dropAll 默认 1006）；acp-connection.test.ts（1006 归类 abnormal 触发重连）、
  mock-acp-ws.test.ts、acp-view.test.tsx 同步。
- D4/D7：新增 ndjson.ts（decodeFrame + NdjsonAssembler）；acp-connection.ts
  接帧走串行解码链 + 行重组；ws-factory.ts 设 binaryType=arraybuffer；
  ndjson.test.ts、acp-connection.test.ts（合帧/跨帧/Blob/换行）回归。
- D5：session/close -> session/delete（acp-connection.sessionDelete + store +
  mock case + 测试）。
- 硬化：acp-connection 定时器改环境无关 API（重连定时器在宿主 window 卸载
  后触发会抛 ReferenceError，全量测试偶发 unhandled error）；mock 回放逻辑
  抽出 mock-acp-prompt.ts（行数红线）。

## 自测

- cd apps/gui && pnpm test && pnpm build && pnpm check:i18n
- cd ../.. && make check（退出码 0）

## 遗留风险

1. **console 与 acp-agent 桥帧契约不匹配（跨 app，超本卡修复范围）**：console
   拨号侧裸写 ndjson 行（dial.rs write_all），acp-agent 侧按 varint 帧读
   （pump.rs read_wire_line -> p2p_protocol read_frame）。真机互连实测 agent
   审计 `conn-denied code="handshake-malformed"`，console 侧 `handshake timeout
   after 10s`。两 app 各自回环自测均绿（console 用裸 ndjson AgentMock、agent
   用 varint 客户端），属典型同源盲区。需两侧负责人拍板统一帧契约后另行修卡；
   修复前 GUI 波不受影响（GUI 只面对 console 的 WS 面），但"GUI->console->
   acp-agent->子进程"全链不可用。
2. agent 侧语义面（initialize 结果、session/update、stopReason 的真实值）未
   经真实 ACP agent 实测：本卡降级路径只有 echo 泵，语义面结论以官方 spec +
   传输面保真为证；接真实 harness agent（pnpm dsh --profile acp）后需复拍 c/e/f。
3. session/new 的 cwd：GUI 暂发 null，非 spec 形状；等 ACP4 桥 cwd 改写
   （scope 监狱）落地后由桥侧兜底，或 GUI 增加工作区配置。
4. 错 token（401）与网络不可达在浏览器约束下同样呈现 1006 空 reason，GUI 无法
   区分归因；页面文案以"连接失败"呈现，token 校验依赖 console 审计日志。
5. 客户端主动断开也收到 1006（console 不回 Close 帧），GUI 的"手动断开"展示
   phase=idle 不受影响，但 closeInfo 若被展示会呈 abnormal——当前 UI 在 idle
   态不展示 closeInfo，无用户可见问题。
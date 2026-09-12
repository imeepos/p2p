# 任务 W2：INTEROP SPEC-GAPS 回写协议规范（消除第三方接入阻断）

## 目标
把 `examples/mininode-python/SPEC-GAPS.md` 里由独立第三方（Python 参考节点）黑盒实测出的规范缺口，
按"**以代码实况为准，只改文档**"回写进 `docs/protocol/**`，使第三方仅凭文档实现即可与本仓节点互通。
最高优先是 GAP-1（阻断级：rendezvous 链路帧前缀与 `wire-format.md §6` 表述不符，照文档实现必被服务端断开）。

## 背景（指针，不是内容）
- 本任务在计划中的位置：收口轮 W2。完成后第三方接入面无阻断级错误，PROTO 规范体系（16 协议 ID）才算真闭环。
- 必读输入（**只给指针，自己读**）：
  - `examples/mininode-python/SPEC-GAPS.md` —— 六条缺口的原始登记（含黑盒观测证据与建议改法），是本任务的输入清单
  - `docs/protocol/spec-charter.md` —— 规范章程（§8 向量、§9 章节骨架与状态口径）
  - `docs/protocol/specs/rendezvous.md` —— §2 线格式、§7 测试向量、§8 实现状态与出处
  - `docs/protocol/wire-format.md` —— §4.4 PeerMismatch、§6 帧格式
  - `docs/protocol/specs/ping.md` —— §3 发起方/应答方关流语义
  - `docs/protocol/quickstart.md` —— §3 引导节点（rendezvous）伪代码段
  - `docs/protocol/registry.toml` + `scripts/check/protocol-registry.sh` —— 注册表机械门禁（改文档后必须仍 PASS）
  - `docs/design/wire-protocol.md` —— 内部设计文档（**本任务不改它**，但可读来确认口径）
- 已核实的事实（协调者已确认，可直接引用，不必重查）：
  - **F1** `crates/p2p-discovery/src/rendezvous/link.rs:159` 用 `(data.len() as u32).to_be_bytes()`；
    测试向量断言同形（同文件 158-161 行）；真实链路实现在 `crates/p2p/src/rendezvous.rs`（`LengthDelimitedCodec`，帧上限 1 MiB，见同文件 `rendezvous_codec`）。
  - **F2** 协议 ID 首帧走 wire-format §6 的 varint 帧（IV1 抓包：`16 2f70...2f31`），**其后每条消息**才是 4 字节大端长度前缀 + protobuf
    （IV1 实测样例 `00 00 00 cb | 0a c8 01 | <200B Register>`，203 = Request 信封全长）。
  - **F3** 默认 namespace 常量 = `p2p-base`，实现在 `crates/p2p/src/assembly.rs:25`（`DEFAULT_NAMESPACE`），
    用于 `RendezvousConfig::new`（同文件 216 行）。
  - **F4** 签名新鲜度窗口 ±300s 已在 `rendezvous.md §4` 写明（GAP-2 的时钟窗口项**已存在，勿重复添加**）。
  - **F5** `docs/protocol/specs/rendezvous.md` 当前 §2 只写"帧 payload = protobuf 消息…一帧一个消息"，
    §8 出处行只写"帧缝（一帧一消息，长度前缀封装）"——两处都没有前缀格式。
  - **F6** `docs/protocol/wire-format.md:93` 写"流语义层的全部字节按帧封装：**无符号 varint 长度前缀 + 定长 payload**"，
    没有排除 rendezvous 控制链路。

## 交付清单（逐条可判定）
对下列 6 条各给出改动；每条必须写明**文档改了什么 + 依据（代码 file:line 或黑盒证据）**：

1. **GAP-1（阻断级）rendezvous 链路帧前缀**
   - `rendezvous.md §2` 明确写死：链路首帧（协议 ID）仍走 `wire-format.md §6` 的 varint 帧；
     **其后每条消息 = 4 字节大端（u32be）长度前缀 + protobuf payload**，长度上限 1 MiB。
     给出至少一个逐字节样例（可用 IV1 实测量，标明来源）。
   - `rendezvous.md §8` 的 link.rs 出处行同步改为真实帧缝描述（u32be 前缀），并**删除**当前"漂移登记：无已知语义漂移"里
     与此冲突的措辞（该行现在失真）。
   - `wire-format.md §6` 增加一句适用范围声明：varint 帧用于流语义层协议；**rendezvous 控制链路（`/p2p-base/rendezvous/1`）
     的消息帧为 u32be 前缀，以 `specs/rendezvous.md §2` 为准**（协议 ID 首帧仍走 varint）。
2. **GAP-2 文档侧**：本条**只改文档**——在 `rendezvous.md`（或 `quickstart.md` 引导节点段）写明
   "lan-only 模式下连私网/回环 bootstrap 也整体跳过注册接线"这一已观测语义，避免读者按"只跳过公网"理解。
   （日志文案失真属代码改动，**不在本任务**，不要动 `crates/**`。）
3. **GAP-3 ping 关流方向**：`ping.md §3` 补明确定义——应答方 handler 返回即整流关闭（收发双向）；
   发起方读到 pong 后**不得再写 FIN/数据**，直接关闭即可；违反时的对端行为（RESET）也写明。
4. **GAP-4 PeerMismatch 断开时机**：`wire-format.md §4.4`（及错误表 `:227` 行附近）写明
   校验失败发生在"握手完成、交换任何应用数据之前"，行为是立即断连 + 显式 PeerMismatch 错误；
   并注明**不应**走 TLS alert 路径（会卡死部分客户端状态机），依据取本仓传输实现。
5. **GAP-5 默认 namespace**：`rendezvous.md` 与 `quickstart.md §3` 登记默认 namespace 常量 `p2p-base`
   （依据 `crates/p2p/src/assembly.rs:25`），并说明第三方要发现"标准节点"应查该 namespace。
6. **GAP-6 半帧悬挂**：`rendezvous.md` 写明读端不足 4 字节前缀时的行为与兜底（依代码/实测取权威值，写清"多久、最终是否回收"；
   若代码中确为依赖空闲连接回收，就照实写）。

**GAP-1 的向量级自证（本任务的适应度检查）**：
- 新增 `docs/protocol/vectors/rendezvous-link-frame.json`，逐字节给出至少两例：
  (a) 协议 ID 首帧（varint 帧）；(b) 一条 u32be 前缀消息帧（用 IV1 实测样例或你按文档规则自造并经本仓实现交叉验证）。
- 生成规则：**只能依据 `docs/protocol/**` 的描述**构造，再用「独立于本仓 Rust 代码的第三方视角」核对
  （例如用 `python3` 按文档规则逐字节拼帧并与实测量/本仓实现对拍）。**禁止**照抄 Rust 源码来"生成"向量再宣称文档自洽。
- 在 `rendezvous.md §7 测试向量` 段登记该新文件一行；`docs/protocol/vectors/README.md` 若按同类登记新文件，同轮补登记。

## 验收标准（全部满足才可报 DONE）
- [ ] 6 条 GAP 逐条有改动（GAP-1 含 3 处文件），每条在汇报里给出 `file:line` 级证据 + 依据出处。
- [ ] 新向量文件在约定路径，格式合法（`python3 -c "import json,sys; json.load(open(...))"` exit 0），
      且**字节级对拍通过**：贴出对拍脚本与你跑出的输出（示例：按文档规则解码 → 断言 equals 实测/实现字节）。
- [ ] 抽检命中：`grep -n "u32" docs/protocol/specs/rendezvous.md docs/protocol/wire-format.md`、`grep -n "p2p-base" docs/protocol/quickstart.md docs/protocol/specs/rendezvous.md` 各至少一处新命中（贴命令与输出）。
- [ ] `bash scripts/check/protocol-registry.sh` → PASS（贴退出码）；`bash scripts/check/line-limit.sh` → PASS。
- [ ] `bash scripts/check/ai-docs-sync.sh` → 退出码 0（若你的改动触发该门禁，按门禁要求补正）。
- [ ] 门槛：`make check` 在**隔离 worktree** 内跑（如主干门禁此刻因 W1 在修仍红，就在 worktree 内只跑受影响面并说明，
      汇报里必须写清你跑了哪些、结果如何；**不得把别人的红记成自己的绿**）。
- [ ] 行数：改动后 `docs/protocol/specs/rendezvous.md` 与 `docs/protocol/wire-format.md` 均 < 300 行（行数红线，贴 `wc -l`）。
- [ ] 收尾：worktree 内 `git rebase main` → push `origin` 分支；**不要自行合并 main**。

## 边界（明确不做）
- **只改** `docs/protocol/**`（`specs/`、`vectors/`、`wire-format.md`、`quickstart.md`、必要时 `builtin-and-versioning.md` 的登记行）。
- **不改** `crates/**`、`apps/**`、`scripts/**`、`Makefile`、`.github/**`（W1 在改 `scripts/check/**`，撞车即事故）。
- **不改** `docs/design/wire-protocol.md`、`docs/coordination.md`、`.agents/skills/**`、`.orchestrator/**`（协调者域）。
- 不新增协议 ID、不改 `registry.toml` 的既有条目语义（若确需新增登记行，先报 NEEDS_CONTEXT）。
- 不做"顺手统一改回 varint"的方案——那要改实现与存量，属契约变更，已裁决**不改实现**。
- 不写"观察/建议/也许"式措辞：规范页只写可判定的 MUST 语句与取值域。

## 接口契约
- Consumes：main @ `4df3eb5`（协调者已 push origin）；`examples/mininode-python/SPEC-GAPS.md` 六条；
  `examples/mininode-python/` 的实测证据（只读，不改）。
- Produces：第三方可独立实现的 `docs/protocol/**` 文档 + `docs/protocol/vectors/rendezvous-link-frame.json`；
  后续任何"第三方接入/互操作"波次卡直接消费这两者。

## 预算与停止条件
- 预算：约 3 轮修复 / 累计工具调用上限约 40 次。
- 立即停止并报 **BLOCKED**：任一 GAP 的"以代码为准"权威值无法从代码或黑盒证据确定（例如 GAP-6 的回收超时在代码里找不到）；
  或你发现文档改法必然要求改代码（契约变更）——此时**不要自行改代码**，列清冲突点回报。
- 立即停止并报 **NEEDS_CONTEXT**：向量对拍出现"文档描述 → 本仓实现"不一致且你无法判定谁对（这正是要上报的漂移，别猜）。

## 工作区与汇报
- 分支 `docs/w2-spec-gaps`，worktree `.worktrees/w2-gaps`（`git worktree add .worktrees/w2-gaps -b docs/w2-spec-gaps 4df3eb5`）。
- 遵循 `AGENTS.md`：只 add 自己范围文件（禁 `git add -A` / `commit -a`）；提交拆分为
  ①GAP-1 帧格式与 wire-format 适用范围 ②GAP-3/4 语义补充 ③GAP-5/6 常量与超时 ④新向量文件（可与你判断的合理粒度合并，
  但"规范页主体"与"向量文件"建议分开提交）。message 用 `type(scope): subject` 并写机理。
- 汇报经 `session_link_send_parent` 回报派发你的主会话。
  派发者显式 id：`session-3373f897-f9c0-4a32-9af2-b9562eecc311`（`send_parent` 异常时改用它定向投递，不重发试错）。
- 汇报状态只用：DONE / DONE_WITH_CONCERNS / BLOCKED / NEEDS_CONTEXT。
- DONE 汇报必须附：变更文件清单（逐文件 `wc -l`）、每条验收标准的命令 + 退出码 + 关键输出、
  六条 GAP 的逐条落点表（GAP 编号 | 改的文件:行 | 依据）、顾虑、结构化复盘（最耗时的是什么 / 本任务书是否提前警告 / 重来怎么做）。

## 早落盘纪律（防宿主静默杀死）
- 开工先建 worktree 并落 `PROGRESS.md`（worktree 根，勿提交），每完成一条 GAP 追加一行。
- 长命令（cargo/make）一律后台 + 日志文件路径，别在无盘上痕迹的状态下长跑。

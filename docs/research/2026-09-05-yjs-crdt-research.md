# Yjs/CRDT 引入调研（2026-09-05，用户推荐引入 Yjs）

## 一、结论先行
推荐**引入，但以 Rust 侧 yrs（Yjs 官方移植）为落地点、friends 域为试点**，分两阶段：
1. Y1 试点（队列中，W 波后）：好友簿从「JSON 文件+文件锁」迁移为 yrs Doc 承载（store API 不变，CLI/GUI 无感），文件持久化存 update/快照；
2. Y2 决策门：试点验收（双进程并发 add 零丢失且语义正确、离线双写自动合并、迁移路径清晰、行数红线达标）通过后，再评估推广至 config/profile 与跨设备同步。

## 二、Yjs 是什么 / 生态现状
- Yjs：TS 生态最成熟的 CRDT 库（Y.Map/Y.Array/Y.Text），配套 y-protocols（sync/awareness）、y-websocket/webrtc/indexeddb。
- **yrs**：官方 Rust 移植（我们的存储层是 Rust），生态活跃：yrs-tokio、yrs-warp（传输集成）、yrs_tree（树结构）——检索证实 2025 年仍在活跃发版。
- 竞品格局：Automerge（Rust+JS 双栖、automerge-repo）、cr-sqlite（CRDT SQLite）、自研 oplog+锁（我们现状）。社区对比普遍结论：Yjs/Yrs 性能与生态最优，Automerge API 更 ergonomic，cr-sqlite 适合已重度 SQLite 的场景。

## 三、与本项目三个痛点的适配分析
| 痛点 | 现状 | CRDT 价值 |
|---|---|---|
| 双进程并发写静默丢失 | 已修（3fb0b59 文件锁） | 锁只串行化不合并；CRDT 给「合并语义」而非「排队语义」 |
| GUI 需刷新才感知（W1 在修） | file-watch+事件通知（进行时） | CRDT doc 天然是变更源，与 W1 事件互补 |
| **跨设备 P2P 同步（战略场景）** | 无 | **决定性收益**：yrs update 交换可直接骑现有 p2p transport（y-sync 协议消息），离线合并免解决冲突——这正是 P2P 产品的原生需求 |

## 四、关键技术判断
1. **CRDT 应整体住在 Rust 侧（yrs），而非前端 yjs**：GUI 的数据编辑本就经 Tauri 命令进 Rust store；CLI 同样链库。前端零 CRDT 依赖（包体/学习成本归零），yjs TS 互操作留作远期（若未来出现前端离线编辑场景）。
2. **消息流不需要 CRDT**：messages/*.jsonl 已是 append-only 行级完整（N2 实测），天然合并安全。需要 CRDT 语义的是「可变状态集」：好友簿（add/remove 需 tombstone）、config/profile（字段级 LWW）、未来分组（IM-T43）。
3. **试点域选择 friends**：冲突最真实（N2/R1 两轮都栽在这）、体量最小、有现成并发 E2E（cli-gui-data-e2e）可直接复用为验收。

## 五、风险与成本
- 数据迁移：现有 friends.json → yrs doc 需一次性迁移 + 兼容读取（store API 不变可对冲）。
- p2p-chat 是共享核心，R1 刚加锁：试点须与 IM 线协调（outbox 修复在途，等其落地）。
- yrs 学习成本 + store 文件 300 行红线（doc 封装独立文件可控）。
- 同机双进程场景 CRDT 属「超配」——但买的是跨设备战略能力，且试点范围小、可回退（store API 不变即回退点）。

## 六、行动项
- Y1（P1，W 波后派）：friends 域 yrs 试点（store API 不变 + 双进程并发 E2E 零丢失 + 离线合并测试 + 迁移兼容）。
- Y2 决策门：Y1 验收后裁决是否推广 config/profile 与 P2P update 交换（骑 p2p transport）。
- 不做：前端引入 yjs、全量数据层 Yjs 化、消息 JSONL 改造。

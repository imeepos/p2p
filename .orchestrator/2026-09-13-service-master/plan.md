# 波次计划：权限统一收口 + 服务总控（2026-09-13，计划 v1 锁定）

> 协调者: session-cd1cdce3-339d-4eb5-9414-d34cf87e51a1 | 来源: 用户直接指令
> 裁决（用户 2026-09-13 单选确认）: D1=统一服务注册表；D2=行为兼容收口（不含 tunnel.visit）；
> D3=持久化 + 安全默认关。事实基础: 同目录 inventory.md（表1/2/3 + 13 开放问题）。

## 0. 已定原则（lead 拍板，A 卡钉成契约，实现卡不得违反）

1. 存储单一真值源 `<data-dir>/services.json`：version 化、tmp+rename 原子写、损坏显式
   报错拒读（沿 authz 存储红线）；data-dir 解析与 authz 数据根同源同根（inventory 问题 12）。
2. 服务清单闭集、只加不删（沿 authz registry 先例）；条目最小字段：
   service_id / enabled / updated_at（描述进代码常量表，不进文件）。
3. 开关三型语义：
   - 布尔型（新显式闸，安全默认关）：serve.llm_share=off、serve.tunnel=off（现状会话态
     默认关，升级为持久化，契约注明行为变化点）。
   - 显式化型（开关 AND 配置双条件，默认 on 不改行为）：net.rendezvous_register / net.relay /
     net.observe / serve.rendezvous_server（消费 public_only）。
   - 收编型：discovery.mdns（迁移期双读：services.json 有条目用之，无则回落 enable_mdns）、
     net.lan_only（修复 GUI 断链：GuiConfig 补字段+消费，GUI/CLI 同语义）。
4. 首批清单 10 项（闭集 v1）：上 3 条所列 8 项 + serve.a2a（显式化型，默认 on，消费点
   acp-agent 启动面）、serve.acp（显式化型，默认 on）。acp-console 泵 / update / repair-helper /
   群聊不入首批（待细化区）。
5. chat 收口（行为兼容三件套，缺一即破坏兼容）：
   a) PEP：入站单聊消息与媒体接收前 check(chat.send / chat.attachment)，Deny（NotBound/
      Expired/BrokenRole/MissingPerm）拒收不入库、审计留痕、不回 wire（沿 acp session.rs:196 先例）；
      挂载 = p2p-chat 定义 CheckGate trait，GUI/CLI 装配处注入 authz 适配器（依赖方向不变：
      chat 不依赖 authz crate）。
   b) auto-bind 补齐：GUI 好友邀请/接受两入口补 default_role 自动绑（对齐 CLI friends.rs:166）。
   c) 存量回填：幂等回填「有好友无绑定 → 绑 default_role」，入口 p2pctl authz import friends
      + GUI/daemon 节点启动时自动执行一次（记审计），确保升级后老好友聊天零中断。
6. CLI/GUI 对等：`p2pctl service list/enable/disable` + GUI 服务面板，同源 services.json；
   cli-parity.tsv service 段登记；生效时机=下次节点启动（不热更，契约明示），服务面板对
   未运行项实时可翻、对运行中项提示重启生效。
7. 独立进程边界：本波不改 acp-agent/repair-helper 判定逻辑；serve.a2a 消费点=acp-agent
   启动时读 services.json（off 即等价 --a2a-disabled）。

## 1. 锁定区（本波两轮）

### 第一轮：A 卡（单飞，契约钉主干）

| 卡 | 分支 | 类型 | 产出（Produces，下游逐字引用） |
| --- | --- | --- | --- |
| A | feat/wsm-a | architecture | ① docs/design/service-registry-design.md（含备选与舍弃理由）② gui-contract.md 新章节 §20（先核下一个空闲 §号）③ authz-role-design.md 追加 Amended 附录（chat PEP 语义+回填+§8 表增行）④ crates/p2p-service 编译桩（类型+store 骨架+单测，独立小提交）⑤ ipc-types.ts §20 方法签名桩 + services.* i18n 键名清单 ⑥ cli-parity.tsv service 段预留行 |

A 卡验收门：主干 `make check` 全绿；下游 B1-B5 任务书可逐字引用其产出（路径+结构精确）；
契约每条要求可判定（写明验收命令）。

### 第二轮：B1-B5 并行（A 已合并 @17176880；合并顺序 B1 → B3 → B2/B5 → B4 → C1）

| 卡 | 分支 | 类型 | scope（文件所有权，独占） | 核心验收 |
| --- | --- | --- | --- | --- |
| B1 | feat/wsm-b1 | backend(rust) | crates/p2p-service/、crates/p2p/（含其 Cargo.toml）、apps/gui/src-tauri/{types.rs,config.rs,state.rs,Cargo.toml}、apps/cli/src/daemon.rs | registry 实装+装配消费 6 服务+lan_only GuiConfig 字段修复；cargo test -p p2p-service -p p2p 绿；GUI lanOnly 开关保存后重启仍生效 |
| B2 | feat/wsm-b2 | backend(rust) | apps/gui/src-tauri/src/{llm_share/,tunnel.rs}、apps/cli/src/llm_share/、apps/acp-agent/src/（含其 Cargo.toml） | serve.llm_share 总闸（off 即不装配借出）+ offer 停借命令补齐；serve.tunnel 持久化闸收编；acp-agent 启动消费 serve.a2a；禁触 state.rs/tunnel.rs 外 src-tauri 文件 |
| B3 | feat/wsm-b3 | backend(rust) | apps/cli/src/{service/,cli.rs,authz/mod.rs 的 service 无关、Cargo.toml}、apps/gui/src-tauri/src/{services.rs,lib.rs}、scripts/check/cli-parity.tsv | service list/enable/disable 三命令 + tauri services_* 命令逐字 §20；cli-parity.sh 0；登记为独立小提交 |
| B4 | feat/wsm-b4 | frontend | apps/gui/src/ 全部 | 服务面板全链路（ipc/mock/i18n/测试）+ remote-access 页 tunnel serve 控件补齐（inventory 问题 2）；截图证据链+console 干净 |
| B5 | feat/wsm-b5 | backend(rust) | crates/p2p-chat/、crates/p2p-authz/src/、apps/cli/src/{authz/,chat/friends.rs}、apps/gui/src-tauri/src/chat.rs | chat PEP 红绿双向（Allow/NotBound/Expired/MissingPerm/读失败拒/审计/附件判定序）+ auto-bind 共享化下沉 p2p-authz + GUI 两入口 + import friends；Produces 接线 diff 供 C1 |
| C1 | feat/wsm-c1 | backend(rust) | apps/gui/src-tauri/src/state.rs chat 段与 daemon.rs 回填段（B1 合并后解冻） | 轻量卡：逐字应用 B5 接线 diff（gate 注入 + 启动回填调用）+ 聚焦测试 |

Consumes 汇总：B1←A 桩④+设计①；B2←设计①+桩④（API 签名）；B3←§20+tsv 预留⑥；B4←§20+ipc-types⑤；B5←authz Amended④；C1←B1+B5。
号段：无 DB 迁移（A 卡已核实）；任何卡发现需要迁移 → 立即 BLOCKED 报号。
依赖行规则：p2p-service 依赖行各卡可在自己分支的独占 Cargo.toml 添加；rebase 时主干已存在同内容行则删除自己的重复行（同内容行 git 自动合并，冲突即平凡行）。
chat 装配注入点（state.rs:110-117 / daemon.rs:81 段）归 B1 独占；B5 严守边界产出 pub 装配 API + 一行接线 diff，由 C1 落地——跨卡接口以 B5 任务书 Produces 为准。

## 2. 待细化区（触发条件驱动，不进本波锁定区）

- 群聊权限粒度 key（authz 闭集无群维度）——触发：群权限需求实际出现。
- tunnel.visit 入 authz 双查——触发：被访滥用案例或用户再裁决（D2 已裁本轮不做）。
- a2a Tauri stub 双轨清理（inventory 问题 3）、acp-agent 生命周期拉起面（问题 10）、
  repair/GUI 授权缺口（表3）、ConnectionGate 底座收口（表2 末行）——打磨候选池。
- 服务热切换（D1 备选 C 的余量）——触发：重启生效被用户实际抱怨。
- authz 存储 yrs 化、踢会话（authz 设计 §14 既有开放问题）。

## 3. 明确不做（本波红线）

- owner 本机管理操作不进角色模型（authz 设计红线 1）；服务开关属 owner 本机面。
- 跨面踢会话（红线 3）；群聊判定；tunnel peer 级判定；acp-agent/repair-helper 判定改造。
- 不引入 DB 迁移；不做运行时热切换；不改线协议。

## 4. 波次验收（主控汇总检查口径）

- 主干 make check 全绿（含 line-limit / cli-parity / panic-hygiene）。
- 行为证据：GUI 服务面板截图链 + CLI service 三命令实跑输出 + chat 未绑 peer 发消息被拒
  且审计有痕 + 绑定好友聊天零中断（回填后）。
- 契约三方对齐：gui-contract §20 ↔ ipc-types.ts ↔ mock 实现；authz Amended ↔ PEP 代码 ↔ 测试锚点。

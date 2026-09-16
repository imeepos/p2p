# 服务注册表设计：services.json 统一服务总控 —— 设计方案 v1

> 状态: v1（波次 2026-09-13-service-master A 卡产出） | 日期: 2026-09-13
> 来源裁决: 用户 D1=统一服务注册表 / D2=行为兼容收口（不含 tunnel.visit）/ D3=持久化 + 安全默认关
> （.orchestrator/2026-09-13-service-master/plan.md §0 已定原则 7 条为不可违反约束，本文为其契约化）
> 下游消费: B1（registry 实装+装配消费）、B2（serve.* 三闸）、B3（CLI/tauri 命令面）、B4（GUI 面板）

## 1. 定位与目标

服务总控 = 本机 owner 对「这台节点对外提供/使用哪些服务」的统一开关面。定位三条：

1. 单一真值源：所有服务开关收敛到 `<data-dir>/services.json` 一个文件；GUI 与 CLI
   同源同语义（plan §0.1、§0.6）。
2. 服务清单闭集、只加不删（沿 authz registry 先例，permissions.rs 数据一致性测试锚定）。
3. owner 本机管理面：服务开关属 owner 本机操作（authz 设计红线 1 语境），不进
   authz 角色/绑定模型，不上 wire。

非目标：不做运行时热切换（生效时机=下次节点启动，plan §0.6）；不做跨机同步；
不改线协议；acp-console 泵 / update / repair-helper / 群聊不入首批（待细化区）。

## 2. 首批服务闭集 v1（11 项，只加不删；ftp 收尾波 FT5 追加第 11 项）

三型语义（与 plan §0.3 逐字一致）：

- **布尔型**：新显式闸，安全默认关。
- **显式化型**：开关 AND 配置双条件，默认 on 不改行为。
- **收编型**：既有开关机制收编进注册表，迁移期双读。

| service_id | 型 | 默认值 | 消费点（file:line 盘点见 inventory.md 表1） | 现状收编方式 |
|---|---|---|---|---|
| `serve.llm_share` | 布尔型 | off | GUI 节点装配借出面（state.rs:121 → llm-share serve/mod.rs:108-137,148-155，offer live 才注册双 handler） | 新显式闸：off 即不装配借出（B2）；offer.json/allowlist 等既有面全部不动，闸在其上 |
| `serve.tunnel` | 布尔型 | off | GUI tunnel.rs:41-56 slot 闸（tunnel_serve_start/stop）；CLI tunnel/serve.rs:84-88 | 现状会话态默认关升级为持久化；**行为变化点：重启后不再回落关闭**（gui-contract §20 注明） |
| `serve.a2a` | 显式化型 | on | acp-agent 启动面（main.rs:36-38 /a2a/1 承载、cli.rs:69 --a2a-disabled） | off 等价 `--a2a-disabled`（B2）；agent 级 enabled+visibility 双条件保留，闸在其上 |
| `serve.acp` | 显式化型 | on | acp-agent /dsh-acp/1 桥准入（session.rs:194-201）+ admin 管理面（apps/cli/src/acp/mod.rs） | off 即不受理非 owner ACP 会话（owner 本机面不经此闸，红线 1）；acp-console 泵不入首批 |
| `net.rendezvous_register` | 显式化型 | on | assembly.rs:188-198,208-210（bootstrap 非空才接线，crates/p2p/src/lib.rs:29-30） | 隐式（bootstrap 列表非空推导）显式化：开关 AND bootstrap 非空双条件，默认 on 不改行为 |
| `net.relay` | 显式化型 | on | assembly.rs:68,75（relay_addrs 非空挂降级链）；CLI daemon.rs:29-34 | 同上：开关 AND relay_addrs 非空 |
| `net.observe` | 显式化型 | on | assembly.rs:54-66（observation_addrs 非空出站上报） | 同上：开关 AND observation_addrs 非空 |
| `serve.rendezvous_server` | 显式化型 | on | assembly.rs:82-84 恒注册 RendezvousServer（crates/p2p/src/lib.rs:49-50 rendezvous_public_only） | 现状每节点恒开收编为显式化：开关 AND public_only 策略双条件，默认 on 不改行为 |
| `discovery.mdns` | 收编型 | 双读：无条目回落 `enable_mdns`（现状默认 true） | GUI state.rs:298 / CLI daemon.rs:26 / 底座 assembly.rs:180-187 | 迁移期双读：services.json 有条目用之，无条目回落 GuiConfig.enable_mdns；gui-config.json 字段保留不迁移 |
| `net.lan_only` | 收编型 | false | assembly.rs:126-132 装配期剥离 bootstrap/relay；CLI daemon.rs:28 | 修复 GUI 断链（inventory 问题 1）：B1 给 GuiConfig 补 lan_only 字段并消费；双读规则同 mdns（services.json 条目优先，缺失回落 GuiConfig 字段），GUI/CLI 同语义 |
| `serve.ftp` | 布尔型 | off | CLI daemon 装配（apps/cli/src/ftp_serve.rs：开关 AND ftp.json root 配置双条件才装配 /ftp/ctrl/1 + /ftp/data/1） | ftp 收尾波 FT5：默认关=零行为变化；ftp.json 缺失/损坏 fail-safe 跳过装配留告警；账号表空 = OpenAuth 仅限信任网（FT6 接 authz 后收敛） |

双读规则（收编型统一）：`services.json` 存在该 service_id 条目 → 条目权威；
缺失 → 回落既有配置字段。用户首次在服务面板/CLI 翻转该开关即落条目，此后旧
字段对可视化面失效（文件仍在，不迁移不删除）。

闭集纪律：新 service_id 必须先登记 §2 表 + crates/p2p-service 常量表 + 一致性
测试锚定（三方对齐，沿 permissions.rs 先例：改表即测试红）；废用只标 deprecated
不删除。

## 3. services.json 文件格式

位置：`<data-dir>/services.json`（与 `authz/` 目录并列，见 §4 数据根）。

```json
{
  "version": 1,
  "services": [
    { "service_id": "discovery.mdns", "enabled": false, "updated_at": 1760000000 }
  ]
}
```

- 信封 `version`（当前 1）：读到其他版本显式报错拒读，升级路径显式化（沿 authz
  store FILE_VERSION 先例）。
- 条目最小字段：`service_id` / `enabled` / `updated_at`（Unix 秒）。服务描述、
  型别、默认值**不进文件**，只进 crates/p2p-service 代码常量表（plan §0.2——
  防文件与代码双真值源漂移）。
- 文件只落**用户显式设置过的条目**；未出现的服务用代码内默认值。新服务加入
  闭集时旧文件无需变更（升级零迁移）。
- `service_id` 不在 §2 闭集 = 损坏语义，显式报错拒读（防配置漂移静默放行）。
- `enabled` 仅接受布尔；`updated_at` 由写路径调用方注入（时钟不进存储层，沿
  authz Clock 注入先例，保证可测性）。

## 4. 读写规则

1. 原子写：同目录临时文件 + sync + rename；失败错误上抛并清理临时文件（沿
   authz store write_atomic 先例）。
2. 损坏显式报错拒读：JSON 解析失败 / 版本不符 / service_id 越闭集，一律显式
   错误上浮，**禁止静默回退空表**（沿 authz 存储红线；读失败时服务开关面显式
   报错，节点装配按默认值 fail-safe：布尔型=关、显式化型=按既有配置语义）。
3. 缺失文件 = 空表首用态（非错误）。
4. data-dir 解析与 authz 数据根**同源同根**（plan §0.1）：services.json 挂在与
   `authz/`（roles.json/bindings.json，gui-contract §18 口径）完全相同的
   data-dir 根下——GUI 侧 = app 数据目录，CLI 侧 = `--data-dir`。装配处必须
   把同一个 data_dir 值传给 authz 与 services 两处读取，禁止各面独立默认值
   （inventory 问题 12 的 repair-helper 人工对齐教训）。本设计不需要 DB 迁移
   （纯文件存储）。
5. 双进程写窗口：GUI/CLI 后写覆盖风险与 authz 同量级，如实声明局限；写路径
   先重读磁盘再按 service_id upsert 合并缩小窗口（沿 authz §6 先例）。

## 5. 生效时机与运行中提示语义

- 生效时机 = **下次节点启动**：装配处启动时读一次 services.json 并消费，
  运行期不重读、不热更（plan §0.6）。
- 对运行中节点：`services_set_enabled` 落盘成功即返回，并携带
  `requiresRestart=true`；服务面板对运行中项提示「重启节点后生效」，对未运行
  项翻转即为终态（下次启动自然生效）。
- 显式化型运行中语义补充：开关只影响下次装配判定；运行中节点的实际状态由
  既有消费面状态接口呈现（如 llm_share_serve_status.assembled），面板不做
  本地推导冒充。

## 6. 备选方案与舍弃理由

| 备选 | 内容 | 舍弃理由 |
|---|---|---|
| A. 并入 GuiConfig | 每服务一个布尔字段塞进 gui-config.json | GuiConfig 双定义漂移已实证（inventory 问题 8：GUI/CLI 字段集不一致，lanOnly 仅 CLI 有）；acp-agent 消费点无 GuiConfig 面（serve.a2a 在独立进程）；「所有服务可开关」的枚举面退化为散字段，GUI/CLI 对等守卫（cli-parity.tsv）无法按清单机械对照；字段无界膨胀违背闭集纪律 |
| B. 散点补开关（D1 落选形态） | 各服务各自加布尔字段/旗标 | 用户裁决 D1 已否（2026-09-13）：重复造轮子、无统一枚举面、语义互不相通 |
| C. services.json yrs 化 | 多设备同步/双进程合并写 | 本波红线不做运行时热切换与线协议变更；authz 存储 yrs 化本身仍是 authz 设计 §14 开放问题，未裁决前服务存储先行 yrs 化引入无先例依赖；单机双进程场景 tmp+rename + 先读后写已满足，局限如实声明（§4.5） |

选定 B（独立文件 services.json）：与 authz 存储同构同纪律，闭集可机械锚定，
CLI/GUI/独立进程三方消费面同一真值源。

## 7. 下游消费矩阵（B 卡逐字引用锚点）

| 卡 | 消费 |
|---|---|
| B1 | §2 闭集表 + §3/§4 读写规则：crates/p2p-service 实装装配消费 6 服务（mdns/lan_only/rendezvous_register/relay/observe/rendezvous_server）+ lan_only GUI 断链修复 |
| B2 | §2 serve.llm_share（off 即不装配借出）/ serve.tunnel（持久化闸收编，注明行为变化点）/ serve.a2a（off 等价 --a2a-disabled）消费语义 |
| B3 | §2 + gui-contract §20 命令表：p2pctl service list/enable/disable + tauri services_list/services_set_enabled；cli-parity.tsv service 段填充 |
| B4 | gui-contract §20 + ipc-types.ts §20 桩 + i18n services.* 键：服务面板（运行中项重启提示，§5） |
| B5 | docs/design/authz-role-design.md「Amended (2026-09-13)」chat PEP 接入（与本文件无耦合，仅同波） |

## 8. 验收命令（契约可判定）

1. 闭集锚定：`cargo test -p p2p-service` 退出码 0（§2 十项逐字锚定测试）。
2. 原子写与损坏拒读：p2p-service store 单测覆盖 tmp+rename / 损坏显式报错 /
   版本不符拒读 / 越闭集 service_id 拒读。
3. 双读语义：B1 交付含「services.json 无 discovery.mdns 条目时 enable_mdns
   权威」回归测试；lan_only GUI 开关保存后重启仍生效（plan §1 B1 验收）。
4. 命令面对等：B3 交付后 `bash scripts/check/cli-parity.sh` 退出码 0
   （service list/enable/disable 三行 mapped 实测）。
5. 全量门禁：`make check` 绿（fmt/clippy/line-limit/panic-hygiene 含内）。

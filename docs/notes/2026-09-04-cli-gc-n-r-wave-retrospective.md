# 2026-09-04 CLI/GC/N/R 四波复盘（协调者 session-61813ecd）

## 一、已完成（12 卡全绿，全部机械验收 + 协调者亲跑复核）

| 波 | 卡 | 交付 | 验收锚点 |
|---|---|---|---|
| CL 对等 | CL1 | apps/cli 脚手架（p2pctl、--json、退出码 0/1/2、每域一模块） | f846671 |
| CL 对等 | CL2 | 节点/配置/档案/对端/身份域 + 双节点环回 E2E | d92f064，RTT 实测 |
| CL 对等 | CL3 | 聊天域（好友/历史/发送/媒体）+ 双跑幂等 E2E | aa18d0c |
| CL 对等 | CL4 | log/metrics/update 域 + cli-parity 守卫入 make check + cli-guide | 26 命令=24 映射+2 豁免 |
| GC 控制通道 | GC1 | GUI 本地控制通道（回环 HTTP+token 600+invoke 白名单+路由上报桥） | 集成 10/10，generate_handler 零新增 |
| GC 控制通道 | GC2 | p2pctl gui 域（status/screenshot/record/navigate/invoke）+ 真机 E2E | e9fa3c0/212771b |
| GC 控制通道 | GC2b | 真机快照回调超时修复（主线程等待互锁） | c76c41a，两连 E2E 绿 |
| N 规范 | N1 | p2pctl-ai-guide 自描述工具面 + ai-docs-sync 门禁入 make check | 31 命令/86 参数比对 |
| N 规范 | N2 | GUI↔CLI 数据面冷一致性+并发写 E2E + 语义契约入 cli-guide | 两连绿 |
| R 加固 | R1 | friends 跨进程文件锁，消灭并发写静默丢失 | 3fb0b59，11 条全量不丢 |
| R 分发 | R2 | p2pctl release 产物构建 + 4/4 冒烟 | 产物 14M |

## 二、待办未做

- 本协调线：无在途卡。
- 其他协调线（不越界）：E10 T17/T18（llm-share 两 crate）在途；IM-T49/T50 在途、IM-T43 好友分组（P2）待做。
- 用户裁决项：GUI 打包分发（dmg/bundle+更新通道）、GUI 对 CLI 写入的实时感知增强、移动端/远端接入面。

## 三、过程暴露的缺陷与待完善

1. **单测绿≠真机绿**（GC2b）：集成测试用帧源注入，真实 WKWebView 快照回调路径零覆盖，主线程等待互锁致真机恒超时。教训已入册：验收必须含真机链路，协调者复跑是最后一道闸。
2. **并发写静默丢失**（R1）：friends.json 无锁 last-write-wins，两进程并发写丢 5/11 条。违反「失败路径留可观测信号」红线，已修。
3. **E2E 自身缺陷**：GIF magic 断言常量笔误（8c16ef7）——守卫脚本也要被守卫。
4. **流程摩擦**：devloop_ledger 30 条上限迫使折叠历史（已按惯例 done 折叠进 note）；账本大载荷经工具传输会截断，改走紧凑 JSON+长度校验。
5. **小尾巴**：apps/gui/src-tauri/Cargo.lock 有一处未提交改动（构建副产物），待归属确认；AGENTS.md 远端名 gitea 与本仓库 origin 不符（已有 ISSUE 登记）。

## 四、过度设计检讨

结论：无明显过度设计，三处刻意克制是正确裁断：
- node 生命周期没上守护进程管理框架，pidfile+端口探测够用（CL2 自选实现）；
- 守卫用 bash+TSV 不引新框架，且实测命令面而非纸面对账（cli-parity/ai-docs-sync 同思路）；
- R2 砍掉 GUI 打包只做 p2pctl 单二进制（砍范围不砍质量：冒烟 4/4 是质量底线）。
- 两份文档（cli-guide/ai-guide）有轻度重叠，合并收益低于折腾成本，不动。
- 「顺手补齐」类（metrics get 缺口、GIF 常量笔误）低成本高价值，保留不视为 scope creep。

## 五、下一步建议（按优先级）

1. **P1 GUI 数据实时感知**：N2 契约实测 GUI 需重启/刷新才感知 CLI 写入——加 file-watch 或显式刷新入口（归 IM/GUI 波，需与其协调文件域）。
2. **P1 GUI 打包分发**：app bundle/dmg + 对接已有 update_check/release 门禁。
3. **P2 AI 接入实战试运行**：拿 ai-guide 让一个真实 AI 会话全流程操作 p2pctl，收集摩擦点回填文档（检验 N1 产出成色）。
4. **P2 IM-T43 好友分组**（IM 线既有排期）。
5. **不做**：插件系统、配置热重载、跨平台 CI 矩阵——现阶段无真实需求，做了就是过度设计。

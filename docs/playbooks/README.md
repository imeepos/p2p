# 远程支持修复 playbook 库（格式 v1，agent 无关）

本目录是 playbook 库（RS P0b T25）。格式规范与 `crates/repair-playbook/src/lib.rs`
的 crate 文档双写——两处改动必须同步。runner 与 shell 白名单均消费本格式。

## 格式规范 v1

### 定位

除推荐 runner 字段外，playbook 不得出现任何 runner 专有绑定（DSH/Codex/Claude
等均禁止）；推荐 runner 允许留空或填写枚举字符串。格式面向任意 agent 的 MCP
工具面（remote-support-design.md §9）。

### 文件结构

文件为 UTF-8 编码的受控 markdown 子集，结构固定：

```text
# Playbook: <名称>                     # H1，必填，必须是首个非空行

- 名称: <名称>                          # 必填，须与 H1 一致
- 问题类别: <类别标识符>                 # 必填，如 performance-slow
- 推荐 runner: <枚举字符串或留空>       # 可选
- 前置条件: <项>                        # 必填，至少一项；多项目占一行或用二级列表逐项
  - <前置条件项>
- 备注: <文本>                          # 可选，可重复

## 红线清单                            # 必填，至少一项

- <整体红线项>

## 步骤 <N> <标题(可选)>               # N 从 1 起严格连续递增

- 说明: <本步做什么>                    # 必填
- 工具: <tool 名>                      # 必填，取自已冻结 P0b 工具面
- 参数: <参数要点>                      # fs_read/fs_list/fs_search 必填；其余可省略
- 命令: <shell 命令单行>                # 工具为 shell_exec 时必填
- 风险档: read|write|danger             # 工具为 shell_exec 时必填
- 验收: <验收命令或可判定标准>          # 必填
- 红线: <该步红线>                      # 必填，可重复形成多条
- 备注: <文本>                          # 可选，可重复
```

### 规则

- H1 必须是文档首个非空行；`## 红线清单` 与 `## 步骤 <N>` 是仅有的两种二级章节；
- 步骤号从 1 起严格连续递增，缺失/非法/跳号均为错误；
- 工具名必须来自 P0b 工具面闭集：sys_snapshot、fs_read、fs_list、fs_search、
  shell_exec、session_report（remote-support-plan.md §3.5）；未知工具引用为错误；
- shell_exec 步骤必须给出单行 `命令`（shell 命令原文）并标注 `风险档`
  （read=只读无副作用；write=可写需审批；danger=高危需审批）；命令/风险档字段
  只允许出现在 shell_exec 步骤；
- 每个步骤必须有 `验收`（验收命令或可判定标准）与至少一条 `红线`；
- 整体红线清单与前置条件至少各一项；所有字段值单行表达（不跨行）；`红线`/
  `备注` 用重复键表达多条；
- 出现未允许的字段为错误。所有校验失败返回带行号的错因，禁止静默忽略；
- 写类命令的风险档由 author 逐步骤标注，helper 侧仍独立重判（以本地重判为准，
  remote-support-plan.md §3.4）。

### shell 命令清单导出

解析结果经 Playbook::shell_commands() 聚合全部步骤中的 shell 命令（含风险档与
步骤号），shell_union() 跨多份 playbook 按出现顺序求并集，供 shell 白名单闭集
（remote-support-plan.md Q7 / T24）直接消费。

## 目录清单

| 文件 | 问题类别 | 状态 |
|---|---|---|
| slow-diagnostics.md | performance-slow 电脑卡慢 | 草案 |
| popup-malware-cleanup.md | popup-adware 弹窗流氓软件 | 草案 |
| c-drive-space-cleanup.md | disk-space-low C 盘空间不足 | 草案 |

三类命令并集即 shell 白名单闭集（Q7）的数据来源。所有草案均标注「P0b 真机演练时
校准」：命令正确性以 Windows 通用知识为准，真机演练后回写校准。

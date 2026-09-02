---
name: self-evolving
description: A self-evolving skill that grows through reflection on mistakes. Use for any task where you want to accumulate experience over time — this skill starts empty and learns from every failure. After each session, you must reflect on what went wrong and feed it back into the skill so the next session is stronger.
---

# Self-Evolving Skill

> 没有预设知识。每一条经验都来自真实踩坑。你的任务：每次用完后反思，把教训喂回来。

## 1. 如何沉淀

**每次完成任务后，花 5 分钟做三件事：**

### 反思

问自己三个问题，答案写进 `notes.md`：

- 哪个坑浪费了最多时间？
- 这个 skill 有没有提前警告我？
- 重来一次我会怎么做？

### 喂回 skill

把反思结果变成 skill 的一部分，直接改文件：

| 你踩的坑 | 喂到哪里 |
|---|---|
| 某个错误反复出现、排查了很久 | `references/known-issues.md` —— 症状 → 原因 → 修法 |
| 学到一个通用经验、下次可以复用 | `references/lessons.md` —— 一句话经验 |
| 犯了某个规则、付出代价 | `references/red-lines.md` —— "禁止 X，因为 Y" |
| 发现一个排查技巧、工具、命令 | `references/techniques.md` —— 什么场景、怎么用 |
| 上游文档/API 有误、无法在 skill 内修复 | 项目根 `ISSUE.md` |

### 写 ISSUE.md

skill 修不了的问题（上游文档错、API 缺、工具 bug）写进项目根 `ISSUE.md`：

- **信息不准**：哪个文档、哪句话、正确的是什么
- **信息缺失**：应该有什么但没有
- **其他**

每条必须具体："X 节说 Y，但实际是 Z。"

## 2. 目录规范

```
self-evolving/
├── SKILL.md              # 本文件：反思流程 + 目录说明（不存经验）
├── docs/                 # 存放文档
├── references/           # 积累的经验（只增不改）
│   ├── lessons.md        # 通用经验，一句一条
│   ├── known-issues.md   # 已知问题：症状 → 原因 → 修法
│   ├── red-lines.md      # 红线：禁止 X，因为 Y
│   └── techniques.md     # 排查技巧、工具、命令
└── scripts/              # 自动化脚本
```

**references/ 写入原则：**

- 只增不改 —— 每条经验都是当时踩坑的现场记录
- 一条经验一行/一段，不要合并
- 经验过时了？在下面加一条新的纠正它，不要删旧的

## 3. 反馈优先级

最值钱的先写：

1. 静默失败，skill 没预警（浪费几小时）
2. 字段名、API 签名、配置 key 写错
3. 措辞模糊，把你引向错误方向
4. 遗漏的排查手法
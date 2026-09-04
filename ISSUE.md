# ISSUE

## AGENTS.md 远端名与实际不符（2026-09-04 T36 检查轮发现）

- **信息不准**：AGENTS.md「收尾四步」一节写「远端名是 gitea 不是 origin」并示例 `git push gitea <分支>`；但本仓库实际只配置了 origin 远端（git@github.com:imeepos/p2p.git），不存在 gitea。账本 note（2026-09-04 12:30 CLI 对等波勘误）已明确「本仓库远端实为 origin（无 gitea），后续任务书统一用 origin」。
- **正确做法**：推送/删远端分支一律用 `git push origin ...`；AGENTS.md 待同步更正。

## apps/cli 全域 cargo fmt --check 存量红（2026-09-04 ACP5 发现）

- **信息缺失/环境漂移**：`cd apps/cli && cargo fmt --check` 在 origin/main（8fc3f4a）上即红——94 处 diff 遍布 chat/gui/peer/node 等既有文件（缺文件末尾换行、单行 if/else 被新版 rustfmt 展开）。本机唯一工具链 rustc 1.98.0 / rustfmt 1.9.0-stable（2026-08-18）相对此前开发时的 rustfmt 行为变严（style edition 默认变化）。根 workspace 的 fmt 门禁（scripts/check/fmt.sh）不覆盖 apps/cli，故 make check 一直绿、无人察觉。
- **正确做法**：apps/cli 需要一次性 `cargo fmt` 收敛（独立 chore(fmt) 提交，勿混入 feature）；收敛前各任务卡的「cd apps/cli && cargo fmt --check 必须绿」在存量文件上无法达成，验收方应以「本卡新增/改动文件零 fmt diff」为准，或先落收敛提交。ACP5 已把本卡新增的 apps/cli/src/acp/** 做到零 diff。

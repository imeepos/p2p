# ISSUE

## AGENTS.md 远端名与实际不符（2026-09-04 T36 检查轮发现）

- **信息不准**：AGENTS.md「收尾四步」一节写「远端名是 gitea 不是 origin」并示例 `git push gitea <分支>`；但本仓库实际只配置了 origin 远端（git@github.com:imeepos/p2p.git），不存在 gitea。账本 note（2026-09-04 12:30 CLI 对等波勘误）已明确「本仓库远端实为 origin（无 gitea），后续任务书统一用 origin」。
- **正确做法**：推送/删远端分支一律用 `git push origin ...`；AGENTS.md 待同步更正。

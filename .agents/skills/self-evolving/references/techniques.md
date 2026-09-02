# Techniques

<!-- 排查技巧、工具命令、调试手法。格式：什么场景 → 怎么用。 -->

- 2026-09-02 全新空目录起项目要用 worktree 流程时：`git worktree add` 需要 HEAD，
  空仓库无 commit 会直接失败。先 `git init -b main` + baseline commit（AGENTS.md/skill/.gitignore），
  再开 worktree；`.worktrees/` 要写进 .gitignore 避免嵌套目录被主树误跟踪。
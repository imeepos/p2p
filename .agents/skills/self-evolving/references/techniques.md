# Techniques

<!-- 排查技巧、工具命令、调试手法。格式：什么场景 → 怎么用。 -->

- 2026-09-02 全新空目录起项目要用 worktree 流程时：`git worktree add` 需要 HEAD，
  空仓库无 commit 会直接失败。先 `git init -b main` + baseline commit（AGENTS.md/skill/.gitignore），
  再开 worktree；`.worktrees/` 要写进 .gitignore 避免嵌套目录被主树误跟踪。
- 2026-09-01 clippy 门禁报 "'cargo-clippy' is not installed" 时：`export PATH="$HOME/.cargo/bin:$PATH" && rustup component add clippy`，装完即可跑 `-D warnings`。
- 2026-09-01 拆提交保可 revert：把后一个提交涉及的 lib.rs 行先临时摘除、验证编译后提交 A，再恢复、验证后提交 B；cargo 对 src/ 下未被 mod 引用的 .rs 文件直接忽略，中间态可安全验证。

# Red Lines

<!-- 格式：禁止 X，因为 Y 发生过。真的付出过代价才记。 -->

- 禁止在已存在 .env 的目录里 `git add .` / `git add -A`：本工作区 .env 存放密钥，
  首次提交必须先写 .gitignore（.env）并使用显式路径 add（2026-09-02 p2p 仓库初始化时规避）。
- 禁止用手工枚举关键词（PASS/SECRET/TOKEN）的 sed 展示 .env：本机 .env 含 *_API_KEY 明文（OPENAI/AMAP/E2B），枚举漏掉 KEY 类导致密钥进对话记录（2026-09-02 实证）。规则：默认把每行 KEY=VALUE 的 VALUE 全部打码，只显式放行非敏感键（HOST/USER/REGION/DOMAIN/URL）。
- 禁止反向同步用 git merge main 留 merge bubble：派单工作流的规则 5 是 rebase main，merge 提交会污染 main 历史还要多占一个提交号（2026-09-02 D-E4 2443366 实例，协调会话裁定不 revert、下不为例）。
- 禁止跨 crate 改动不先报协调会话：派单声明的 crate 范围是边界，噪声源在别的 crate 接线层也不能直接动手——先回报再改（2026-09-02 D-E4 6a4df8c 触及 crates/p2p facade 被点名，本次因无并发冲突豁免）。
- 禁止依赖 .env 的部署脚本只在主树验证：.env 被 gitignore，worktree 里没有副本，脚本在 worktree 内运行必死于凭据加载——这类脚本必须有显式外部指定入口（如 DEPLOY_ENV_FILE=<主树>/.env）并在头部写明（2026-09-02 ECS 部署实录，静态自检 grep .env 当场拦下）。
- 禁止 `git commit -m $(cat <<'EOF' ... )`：命令替换被词切分，commit 只收到第一个词却静默不报错（HEAD 不动、文件留在暂存区），本次靠提交后核对才发现空跑。规则：多行 message 一律 `git commit -F - <<'MSG' ... MSG`，提交后必须核对 git log（2026-09-02 E4 K 会话实录）。

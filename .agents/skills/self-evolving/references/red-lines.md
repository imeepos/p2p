# Red Lines

<!-- 格式：禁止 X，因为 Y 发生过。真的付出过代价才记。 -->

- 禁止在已存在 .env 的目录里 `git add .` / `git add -A`：本工作区 .env 存放密钥，
  首次提交必须先写 .gitignore（.env）并使用显式路径 add（2026-09-02 p2p 仓库初始化时规避）。
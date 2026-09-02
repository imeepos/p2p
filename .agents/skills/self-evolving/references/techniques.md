# Techniques

<!-- 排查技巧、工具命令、调试手法。格式：什么场景 → 怎么用。 -->

- 2026-09-02 全新空目录起项目要用 worktree 流程时：`git worktree add` 需要 HEAD，
  空仓库无 commit 会直接失败。先 `git init -b main` + baseline commit（AGENTS.md/skill/.gitignore），
  再开 worktree；`.worktrees/` 要写进 .gitignore 避免嵌套目录被主树误跟踪。
- 2026-09-01 clippy 门禁报 "'cargo-clippy' is not installed" 时：`export PATH="$HOME/.cargo/bin:$PATH" && rustup component add clippy`，装完即可跑 `-D warnings`。
- 2026-09-01 拆提交保可 revert：把后一个提交涉及的 lib.rs 行先临时摘除、验证编译后提交 A，再恢复、验证后提交 B；cargo 对 src/ 下未被 mod 引用的 .rs 文件直接忽略，中间态可安全验证。

- 2026-09-02 Rust 依赖 API 核对：cargo fetch 后直接 grep ~/.cargo/registry/src/<registry>/<crate>-<ver>/src 源码确认真实签名（registry 域名目录用 ls -d $HOME/.cargo/registry/src/*/ 取），比查 docs.rs 快且与实际版本一致。
- 2026-09-02 冲突的 Cargo.lock 处理：解掉 Cargo.toml 冲突后 rm Cargo.lock && cargo fetch 让其按新清单整体重生，再 git add，不手工解 lock 冲突。
- 2026-09-02 macOS 无 coreutils timeout 命令，限时跑命令交给外层工具超时参数，不要写 timeout 120 cargo test。
- 2026-09-02 协调多会话并行开发时：协调会话在主树留未提交修改会被 worker 会话收尾的 `git add -A` 卷进它的提交（p2p 项目 1971e69 实例）。主树要么保持 clean，要么编辑完立即 `git add <具体文件> && git commit`；协调文档改动走"编辑+提交同轮完成"。
- 2026-09-02 文档与代码对齐（V 类任务）：写进文档的每个常量当场标注 `文件:行号`，收尾用一次 grep 常量名批量核对出处行号；行号引用要落在语义块起点（struct/const 行），别落在注释或空行上。
- 2026-09-02 run_code 里 git commit 带多行长 message：message 含单引号会被 bash -c 外层包裹炸出 "unexpected EOF"，把 message 用 write 写到 /tmp/x.txt 再 `git commit -F /tmp/x.txt`。
- 2026-09-02 长任务跨并行会话：收尾前别信任务开始时的扫描结果——期间 main 可能已前进（本次会话中段进了 87e8683/75d8ad8 两个提交）；rebase 后要 diff 一下新增文件，新文档可能改变已写好的结论（实例：wire-protocol.md v1 把签名未覆盖 TTL 按现状冻结，审查报告须补冲突说明再合并）。
- 2026-09-02 rebase 后不重跑全量测试也可信迁移绿色结论：`git diff --stat <已验证commit> <合并后commit>`，若 diff 不含本任务任何产物（本任务子树为空）则两树在本任务范围内逐字节一致，绿色结论 1:1 转移；diff 里只应出现并行会话的新文件。
- 2026-09-02 run_code 多行 bash -c 里 echo 文案含裸 ")"（如 "== 4) 前缀 =="）会炸出 syntax error near unexpected token 并中断后续行；验证步骤文案避免裸括号，或把每步拆成独立调用。
- 2026-09-02 对同一文件先跑过 cargo fmt 再用 edit 工具会报 "file changed since it was read"——fmt 改写文件使读快照失效；编辑前重读一次该文件即可。把 fmt 放在"编辑完最后一步"或"编辑前"执行，别夹在编辑序列中间。
- 2026-09-02 run_code 调用必须同时带 code 与 description 两个参数，漏 description 连环报 "invalid arguments: missing required property description"（本会话连犯多次才定位是外层调用缺参，与代码内容无关）；另外 binding 参数里传 undefined（如可选的 workdir）报 "binding arguments must be lossless JSON"，可选参数要按条件省略 key 而非传 undefined。
- 2026-09-02 bash 管道吞退出码：`make check | tail` / `cargo test | tail` 报告的 exit 恒 0，连 `bash: cargo: command not found` 都显示 [exit 0]（本次实录）。门禁结论必须显式收退出码：`make check > log 2>&1; echo exit=$?`，再从 log 取摘要。
- 2026-09-02 edit/run_code 里嵌 Rust 代码片段时用模板字符串包裹，别用双引号 JS 字符串——内嵌的双引号要逐层转义极易错；也别把 Rust 字符串改成单引号（Rust 无单引号字符串字面量，format!('...') 直接语法错误，本会话返工实录）。
- 2026-09-02 长任务中途发现自己的 worktree/本地分支凭空消失：先 `git log --oneline main` + `git worktree list` + `git ls-remote`，大概率已被协调会话验收合入（squash 成新 hash）并执行收尾四步清理——代码在 main 上，别当事故排查（2026-09-02 E4 hairpin 实录：我推的 ff0388d 被合为 0f1c73b，diff 核对逐字节一致）。

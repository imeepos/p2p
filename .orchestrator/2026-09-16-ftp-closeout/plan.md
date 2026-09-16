# FTP 收尾波计划（2026-09-16-ftp-closeout）

来源：2026-09-16 复盘（下一步建议 P0-P2 + 暴露缺陷）。用户指令：先检索社区最佳实践
定制方案，按计划逐项执行，每项完成即验证提交；worktree 隔离，合并后清分支。

## 社区实践检索结论

1. **上传残留**（关键决策）：ProFTPD HiddenStores 模式 = 上传先写同目录隐藏临时
   文件，成功后原子 rename 到目标，失败自动清临时文件——目标路径永不出现半截
   文件，全失败模式统一收敛（来源：ProFTPD 邮件列表 HiddenStores 讨论；
   对照 vsftpd/pyftpdlib 默认保留 partial，属易积垃圾的次选）。采纳 HiddenStores。
   APPE 保持直写目标（追加语义=断点续传友好，vsftpd 先例）。
2. **大目录列表**：FTP 无分页标准（pyftpdlib/PHP rawlist 巨目录问题佐证）；
   防御做法 = 条目上限显式拒绝（克制版替代分页），超限 552 断流。采纳。

## 任务分解（单 worktree feat/ftp-closeout 串行，每任务独立 commit 可单独 revert）

- FT1 删 FtpError::DataAborted 死变体（复盘缺陷 4）
- FT2 HiddenStores：STOR 走同目录 `.<name>.p2p-ftp-partial` 临时文件，成功原子
  rename、失败自动清；APPE 直写目标。测试：成功后无临时文件；超限/断流后目标与
  临时文件均不存在；APPE 不受影响
- FT3 LIST 条目上限：FtpConfig.max_list_entries（默认 10_000），超限 552 显式
  报错。测试：超限拒绝、限内正常
- FT4 p2pctl ftp 域：ls/get/put/mkdir/rmdir/delete/pwd/cd（一次性连接：
  bootstrap→connect→op→exit）；同步 cli-parity 机制与 docs/ops/p2pctl-ai-guide.md
  （ai-docs-sync 门禁硬约束）
- FT5 serve.ftp 服务开关：p2p-service 闭集 +1（Boolean 默认关，零行为变化），
  anchor 测试同步；daemon 装配接线（开关 AND ftp.root 配置双条件才装配）
- FT6 authz 收编：Permission 闭集 + file.read/file.write（anchor 测试同步）；
  p2p-cli 桥接 FtpAuthzAuthenticator（peer+账号→gate 判定），daemon 装配消费

FT5/FT6 触闭集契约（改表即测试红），本计划文件即裁决依据；两闸均默认关=零行为变化。

## 验证与合并

每任务：cargo test -p <crate> 绿即 commit（type(ftp): ...）。波次收尾全量
make check → rebase origin/main → 主树 ff-only → push → 清 worktree/分支 →
SESSIONS.md 释放行。

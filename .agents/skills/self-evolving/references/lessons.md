# Lessons

<!-- 一条经验一行。格式：当 X 发生时，修复是 Y。skill 没提前警告我。 -->

_none yet — be the first._
- 2026-09-01 harness 的 edit 工具按文件路径记账：主树读过的文件，在 worktree 下的同名文件仍需先 read 再 edit，否则直接拒绝。
- 2026-09-01 未来/新版本工具链（rustc 1.98）的 std API 可能与训练记忆不符，编译错误时先读编译器诊断本身（它会给新签名提示），别按旧 API 硬猜。

- 2026-09-02：跨语言脚本里用 python/ruby 批量做文本替换时，replace 未命中会静默成功返回——改数与改后内容必须断言或复查；Rust 场景优先用结构化 edit 工具按精确 old_string 改。
- 2026-09-02：并发驱动任务"等待外部请求"与"推进内部 IO"必须放在同一个 select 中，任何一处单独 await 都可能饿死另一方（yamux 驱动实证）。
- 2026-09-02：封装库把上游 poll API 包成 AsyncRead/AsyncWrite 时，读写长度必须以调用方缓冲为上限，不得自定常量（quinn 适配器实证）。
- 2026-09-02：tokio::join! 的两个 future 各自可变借用同一变量必 E0499；小缓冲管道（duplex 4KB）下两端同时 write 还会互堵死锁。测试按阶段"一写一读并发"设计，别追求全双工对称。
- 2026-09-02：expect_err/unwrap 要求 Ok 类型实现 Debug；Box<dyn Trait> 没有 Debug，改用 match 取 Err 并 panic，别给协议类型强加 Debug。
- 2026-09-02：停车式协议（首个请求挂起等配对方）必须保证每个请求帧最终都有响应帧（成功 Bound 或显式 Reject），否则客户端只能靠超时兜底，等于没有错误信号；Bound 应在配对成功时同时发给两侧。
- 2026-09-02：客户端读回包遇 EOF（对端干净关流）要映射为显式 LinkClosed，落到"意外回包"分支会把排查引向协议层假问题。
- 2026-09-02：验收测试与构建会改写 Cargo.lock，收尾 remove worktree 前先 diff 锁文件；漂移仅多版本规范化时 checkout -- 丢弃即可。
- 2026-09-02：run_code 里多行 bash 命令放进 "..." 双引号字符串会因换行直接 JS 语法错误（Expected ',', got 'ident'）；用数组 join(" && ") 拼接，长 commit message 用 heredoc（git commit -F - <<'MSG'）。
- 2026-09-02：文档与代码对齐任务先 grep 常量与结构再落笔，别信设计稿——p2p design §6 写 sha256(protobuf(公钥))，实现是原始 32 字节公钥哈希（p2p-identity lib.rs）。
- 2026-09-02：glob "crates/*/src/*.rs" 只匹配单层目录，mod 子目录（如 rendezvous/）会漏；枚举 crate 源码用 "crates/**\/*.rs" 再读，避免误判"文件不存在"。
- 2026-09-02：要从文件里抽出保留段时，先从原 ref（git show HEAD:path）抽出，再覆写文件；先覆写再从新文件 sed 抽段，抽出来必然是空段或错段（X 会话拆测试子模块踩过，靠 git show HEAD 无损重做）。
- 2026-09-02：交接描述里"当前 main 是绿的"要用门禁实测复核，别照抄；fmt 门禁上线即暴露 32 文件存量违规，红的是存量不是门禁。
- 2026-09-02：tokio::join!(a, b) 求值即等待、返回 tuple，不是 Future；直接塞给 timeout/自定义 expect_within 报 E0277 "tuple is not a future"，包一层 async { tokio::join!(..) } 即可。
- 2026-09-02：clippy never_loop：loop 体每个分支都 return/panic 时删掉 loop 直接 match；若本意是"跳过不合意事件继续等"，就把该分支写成显式 continue，别让每个分支都退出。
- 2026-09-02：并行会话进行中 main 会新增门禁（本日中途上了 make check）；收尾验收跑仓库门禁本身（make check），别只跑任务书里写死的三条命令，且验收前再 rebase 一次 main。
- 2026-09-02：挂死诊断三板斧实测有效：ps 找测试进程 pid → macOS sample <pid> 2 出线程栈 → 看到 kevent+park 即"全任务 pending 等 IO"，逐个检查 duplex EOF 传播链（见 known-issues split BiLock 条）。

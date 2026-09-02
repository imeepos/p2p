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

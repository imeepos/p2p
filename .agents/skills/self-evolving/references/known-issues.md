# Known Issues

<!-- 格式：症状 → 原因 → 修法。排查超过 5 分钟的 bug 才值得记。 -->

_none yet — be the first._
## 2026-09-01 rustc 1.98 io::Error API 变化（downcast_ref 消失）
症状：`err.downcast_ref::<E>()` 报 E0599 "no method named downcast_ref"；`err.into_inner()` 返回 `Option<Box<dyn Error + Send + Sync>>` 而非裸 Box。
原因：本机工具链 rustc 1.98.0 (2026-08) 的 std::io::Error API 已演进，旧写法全部失效。
修法：`err.into_inner()` 先处理 None，再对 Box 用稳定的 `downcast::<E>()`（Result<Box<E>, Box<dyn Error>>）；封装成 flatten_io 一类的还原函数，测试与库共用。

## 2026-09-01 rustc 1.98 unused_mut 误报与 E0596 死锁
症状：`let (mut tx, mut rx) = duplex(..); write_frame(&mut tx, ..)` 报 unused_mut 警告，但按建议删掉 mut 立即报 E0596 cannot borrow as mutable——二者矛盾，-D warnings 下无解。
修法：保留 mut，在 let 语句前加 `#[allow(unused_mut)]` 并注释说明矛盾原因；不要全文件 allow。

## 2026-09-02 p2p 内核传输（Rust/cargo 生态）

- cargo fetch 拉不到清单未引用的 crate：quinn/yamux/snow/rcgen 只有写进 Cargo.toml 后 fetch 才下载。症状：registry/src 里 grep 不到版本目录。先改清单再 fetch。
- yamux 0.13（paritytech）无 Control/async API，纯 poll 模型（poll_new_outbound / poll_next_inbound）；连接必须被持续轮询才会冲刷流写缓冲。驱动任务等待开流请求时若阻塞在 mpsc.recv() 上，对端流写入会永久卡死——须用 tokio::select! 单点驱动（连接轮询与开流请求竞争）。
- quinn RecvStream::poll_read(cx, &mut [u8]) 会把整个传入切片注册为 ReadBuf，调用方（tokio AsyncRead 适配器）必须先用 ReadBuf::remaining() 截断长度，否则 put_slice 断言 panic（"buf.len() must fit in remaining()"）。
- snow write_message 输出缓冲必须容纳 token 开销：XX msg2 = e(32)+s(48)+tag(16)+payload，只留 payload+64 会得到 Err(Error::Input)（"snow: input error"），极易误判为解密失败。
- tokio-util 0.7 没有 copy_bidirectional；它在 tokio::io 下且签名是两条流（a->b 与 b->a），单流自回环要用 io::split + io::copy + shutdown。
- rustls 0.23.43：ClientConfig with_client_auth_cert / ServerConfig with_single_cert 收 PrivateKeyDer（由 provider 加载），不再收 Arc<dyn SigningKey>；danger 校验器在 client::danger / server::danger；quinn 的 conn.peer_identity() 返回 Box<dyn Any>，downcast 目标是 Vec<rustls::pki_types::CertificateDer>。

## 2026-09-02 p2p 中继穿透（Rust/cargo 生态）

- cargo clippy 不认 --message-format（cargo test 可以）：接在 -- 后报 "Unrecognized option: 'message-format'"，clippy-driver 参数路径不同。clippy 要短输出直接看默认格式。
- prost derive 手写 oneof 信封：字段属性 #[prost(oneof = "relay_msg::Kind", tags = "...")] 里的模块路径是字符串，必须与实际 pub mod relay_msg 路径逐字一致，写错只在 decode/编译期报隐晦错误。
- 只新增了 impl 块文件却忘在 lib.rs 声明 mod x;：文件不在模块树，报错是调用处 E0599 "method not found in Arc<T>"，离缺失点很远；新增文件先补 mod 声明。
- 本仓 thiserror 1.x/2.x 多版本并存，cargo 任何一次构建都会把锁文件成员依赖行 "thiserror" 规范化为 "thiserror 1.0.69"（多版本消歧），导致 git worktree remove 报 "contains modified files"。修法：git diff 确认仅此漂移后 checkout -- Cargo.lock 再 remove，勿直接 --force。
- prost 手写消息要求派生 prost::Oneof 的 enum 用 #[prost(message, tag = "n")] 标注每个变体，tags 列表要与 tag 集合一致，漏一个 tag 解码未知字段时静默跳过。

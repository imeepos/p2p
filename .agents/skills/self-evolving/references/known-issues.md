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

//! rd-fs：隔离根目录文件服务（remote-desktop-plan §2.3/§3.3，M5）。
//!
//! [FsService] 把所有相对路径操作锁在 root 内：wire 层 [rd_wire::sanitize_rel_path]
//! 拒绝对路径/`..`/NUL，本层再对真实路径做 canonicalize 前缀校验（防符号链接逃逸）。
//! 传输状态机：upload=viewer→host 顺序写（offset 严格递增，断点续传 = 已存在部分文件
//! 续写）；download=host 顺序读发回。

mod service;
mod transfer;

pub use service::{FsError, FsService};
pub use transfer::TransferHandle;

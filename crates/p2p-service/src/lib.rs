//! 服务注册表（service-registry-design）：`<data-dir>/services.json` 单一真值源，
//! 首批 10 服务闭集只加不删（§2）、version 信封 + tmp+rename 原子写 + 损坏显式
//! 拒读（§3/§4，沿 p2p-authz 存储红线）、data-dir 与 authz 数据根同源同根。
//! 本 crate 只持有闭集常量表与存储骨架；装配消费（6 服务接线/lan_only 修复）
//! 属实现卡 B1，命令面属 B3——本桩不做任何业务面接线。

pub mod registry;
pub mod store;

#[cfg(test)]
mod registry_tests;

pub use registry::{effective_enabled, ServiceEntry, ServiceId, ServiceKind, ServiceRegistry};
pub use store::{load_registry, save_registry, ServiceStoreError, FILE_VERSION, SERVICES_FILE};

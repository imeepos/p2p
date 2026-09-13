//! 服务注册表（service-registry-design）：`<data-dir>/services.json` 单一真值源，
//! 首批 10 服务闭集只加不删（§2）、version 信封 + tmp+rename 原子写 + 损坏显式
//! 拒读（§3/§4，沿 p2p-authz 存储红线）、data-dir 与 authz 数据根同源同根。
//! 命令面属 B3；节点装配消费经 switches facade（B1），本 crate 不接业务面。

pub mod registry;
pub mod store;
pub mod switches;

#[cfg(test)]
mod registry_tests;

pub use registry::{effective_enabled, ServiceEntry, ServiceId, ServiceKind, ServiceRegistry};
pub use store::{load_registry, save_registry, ServiceStoreError, FILE_VERSION, SERVICES_FILE};
pub use switches::NodeServiceSwitches;

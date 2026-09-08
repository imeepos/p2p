//! LSG1 测试（PR 轨会签映射：测试名 ↔ 契约条目）：matrix = §16.1 serde 全形状，
//! smoke = 桩数据命令冒烟与 §16.2 语义红线（默认拒绝、真实成本必填、幂等 reqId），
//! share = §16.6 v13 命令面（provider/台账/allow 透传/redeem 参数错/serve 槽位）。

mod common;
mod matrix;
mod serve;
mod share;
mod smoke;

//! E6 稳定性回归（300 行红线拆分承接）：回收/保活/既有语义三簇用例。
//! 消融点：1 reclaim.rs 空闲拆桥分支 / 2 keepalive.rs spawn_keepalive /
//! 3 control.rs control_loop timeout 包装——删任一处对应用例即红。

mod keepalive;
mod reclaim;
mod support;

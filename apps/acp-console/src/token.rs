//! 本地面鉴权 token（设计 §6）：uuid v4 simple 形态（122 bit 随机），
//! 启动时生成一次，WS 与 status 端点共用；仅经 stdout ready 行与查询面分发。

use uuid::Uuid;

pub fn new_token() -> String {
    Uuid::new_v4().simple().to_string()
}

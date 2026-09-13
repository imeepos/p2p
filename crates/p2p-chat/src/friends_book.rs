//! 好友簿静态读（无节点装配面）：`p2pctl authz import friends`、daemon/GUI
//! 启动回填等离线消费方使用；读路径与 Store::friends_list 同一 yrs 合并语义。

use std::path::Path;

use crate::model::{ChatError, ChatFriend};
use crate::store_friends::FriendsBook;

/// 读取 <data_dir>/chat/friends.json 全量好友（文件缺失=建头行空簿，语义同装配读）。
pub fn friends_book(data_dir: &Path) -> Result<Vec<ChatFriend>, ChatError> {
    let path = data_dir.join("chat").join(crate::store_friends::FILE_NAME);
    Ok(FriendsBook::load(&path)?.list())
}

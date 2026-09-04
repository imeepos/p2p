//! 本机对外声明地址（chat/advertised.json）：常驻 serve 进程写、一次性命令读。
//! 解决 CLI 一次性命令监听端口随机导致 INVITE 帧携带即弃地址、对端同意后
//! 无法回拨的问题；无声明文件时回退节点当前 listen_addrs。

use std::path::PathBuf;

use crate::store_io::{atomic_write, load_json_file};

/// 声明地址文件名（chat 数据目录下）。
pub(crate) const ADVERTISED_FILE: &str = "advertised.json";

/// 读声明地址；文件缺失或损坏返回空（调用方回退 listen_addrs）。
pub(crate) fn load_advertised(chat_dir: &PathBuf) -> Vec<String> {
    load_json_file(&chat_dir.join(ADVERTISED_FILE), ADVERTISED_FILE)
}

use crate::store::Store;

impl Store {
    /// 读声明地址（缺失/损坏 = 空表，调用方回退 listen_addrs）。
    pub(crate) fn advertised_load(&self) -> Vec<String> {
        load_json_file(&self.advertised_path, ADVERTISED_FILE)
    }

    /// 覆盖写声明地址（serve 启动时以当前 listen_addrs 刷新）。
    pub(crate) fn advertised_save(&self, addrs: &[String]) -> Result<(), std::io::Error> {
        let bytes = serde_json::to_vec_pretty(addrs).map_err(std::io::Error::other)?;
        atomic_write(&self.advertised_path, &bytes)
    }
}

/// 写声明地址（serve 启动时以当前 listen_addrs 覆盖）。
pub(crate) fn save_advertised(
    chat_dir: &PathBuf,
    addrs: &[String],
) -> Result<(), std::io::Error> {
    let bytes = serde_json::to_vec_pretty(addrs).map_err(std::io::Error::other)?;
    atomic_write(&chat_dir.join(ADVERTISED_FILE), &bytes)
}

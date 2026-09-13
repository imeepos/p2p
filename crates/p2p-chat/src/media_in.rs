//! 入站媒体接收（core.rs 行数红线拆分）：MEDIA_BEGIN 校验 → 逐 MEDIA_CHUNK
//! 写入 tmp 文件 → rename 落盘；任何损坏路径删 tmp 并显式报错（不留半态文件）。

use std::path::PathBuf;

use p2p_mux::BoxedStream;
use p2p_protocol::read_frame;
use tokio::fs;
use tokio::io::AsyncWriteExt;

use crate::core::ChatCore;
use crate::model::{sanitize_name, ChatMediaMeta};
use crate::wire::{MediaBegin, MEDIA_BEGIN, MEDIA_CHUNK};

impl ChatCore {
    /// 入站收媒体：MEDIA_BEGIN 校验 → 逐 MEDIA_CHUNK 写入 tmp 文件 → rename 落盘。
    pub(crate) async fn receive_media(
        &self,
        stream: &mut BoxedStream,
        peer: &str,
        msg_id: &str,
        meta: &ChatMediaMeta,
    ) -> std::io::Result<PathBuf> {
        let frame = read_frame(stream).await?;
        let Some((&kind, payload)) = frame.split_first() else {
            return Err(std::io::Error::other("媒体头帧缺类型头"));
        };
        if kind != MEDIA_BEGIN {
            return Err(std::io::Error::other(format!(
                "期望 MEDIA_BEGIN(0x02)，收到 {kind:#04x}"
            )));
        }
        let head: MediaBegin = serde_json::from_slice(payload)
            .map_err(|e| std::io::Error::other(format!("媒体头 JSON 非法：{e}")))?;
        if head.len != meta.size {
            return Err(std::io::Error::other(format!(
                "媒体长度不一致：头 {} ≠ 信封 {}",
                head.len, meta.size
            )));
        }
        let dir = self.store.media_peer_dir(peer)?;
        let final_path = dir.join(format!("{msg_id}_{}", sanitize_name(&meta.name)));
        let tmp = dir.join(format!(".tmp-{msg_id}-{}", std::process::id()));
        let mut file = fs::File::create(&tmp).await?;
        let mut received: u64 = 0;
        while received < head.len {
            let frame = read_frame(stream).await?;
            let Some((&kind, payload)) = frame.split_first() else {
                let _ = fs::remove_file(&tmp).await;
                return Err(std::io::Error::other("媒体分片缺类型头"));
            };
            if kind != MEDIA_CHUNK {
                let _ = fs::remove_file(&tmp).await;
                return Err(std::io::Error::other(format!(
                    "期望 MEDIA_CHUNK(0x03)，收到 {kind:#04x}"
                )));
            }
            received += payload.len() as u64;
            if received > head.len {
                let _ = fs::remove_file(&tmp).await;
                return Err(std::io::Error::other("媒体超过声明长度，断流"));
            }
            file.write_all(payload).await?;
        }
        file.flush().await?;
        drop(file);
        fs::rename(&tmp, &final_path).await?;
        Ok(final_path)
    }
}

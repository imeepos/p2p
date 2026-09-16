//! 客户端：VDriveClient——每操作一根流（请求 → 应答 → 关流），无会话态。
//!
//! 实现 [FsBackend]，因此 WebDAV 挂载桥对「本地后端」与「远端客户端」
//! 同型：dav 层不区分两边，itest 可直接对拍。

use std::sync::Arc;

use async_trait::async_trait;
use p2p::Node;
use p2p_identity::PeerId;
use p2p_protocol::{read_frame, ProtocolId};

use crate::backend::FsBackend;
use crate::error::{ErrorKind, VDriveError};
use crate::wire::{
    decode_reply, write_request, Entry, ReplyData, Request, StatFs, MAX_CHUNK, PROTO_FS,
};

#[derive(Clone)]
pub struct VDriveClient {
    node: Arc<Node>,
    peer: PeerId,
    proto: ProtocolId,
}

impl VDriveClient {
    pub fn new(node: Arc<Node>, peer: PeerId) -> Result<Self, VDriveError> {
        Ok(Self {
            node,
            peer,
            proto: ProtocolId::new(PROTO_FS)
                .map_err(|e| VDriveError::new(ErrorKind::Io, format!("protocol id: {e}")))?,
        })
    }

    pub fn peer(&self) -> PeerId {
        self.peer
    }

    /// 一次 RPC：开流 → 请求（含可选数据帧）→ 应答（read 另收数据帧）。
    async fn rpc(
        &self,
        req: Request,
        data: Option<&[u8]>,
    ) -> Result<(ReplyData, Option<Vec<u8>>), VDriveError> {
        self.node
            .connect(self.peer)
            .await
            .map_err(|e| VDriveError::new(ErrorKind::Io, format!("dial: {e}")))?;
        let mut stream = self
            .node
            .new_stream(self.peer, self.proto.clone())
            .await
            .map_err(|e| VDriveError::new(ErrorKind::Io, format!("open stream: {e}")))?;
        write_request(&mut stream, &req, data)
            .await
            .map_err(|e| VDriveError::new(ErrorKind::Io, format!("send request: {e}")))?;
        let frame = read_frame(&mut stream)
            .await
            .map_err(|e| VDriveError::new(ErrorKind::Io, format!("read reply: {e}")))?;
        let reply = decode_reply(&frame)?;
        if matches!(req, Request::Read { .. }) {
            let bytes = crate::wire::read_data_frame(&mut stream)
                .await
                .map_err(|e| VDriveError::new(ErrorKind::Io, format!("read data: {e}")))?;
            return Ok((reply, Some(bytes)));
        }
        Ok((reply, None))
    }
}

fn expect_entry(reply: ReplyData) -> Result<Entry, VDriveError> {
    match reply {
        ReplyData::Entry(e) => Ok(*e),
        other => Err(VDriveError::new(
            ErrorKind::Io,
            format!("unexpected reply kind: {other:?}"),
        )),
    }
}

fn expect_statfs(reply: ReplyData) -> Result<StatFs, VDriveError> {
    match reply {
        ReplyData::StatFs(s) => Ok(s),
        other => Err(VDriveError::new(
            ErrorKind::Io,
            format!("unexpected reply kind: {other:?}"),
        )),
    }
}

fn expect_entries(reply: ReplyData) -> Result<Vec<Entry>, VDriveError> {
    match reply {
        ReplyData::Entries(v) => Ok(v),
        other => Err(VDriveError::new(
            ErrorKind::Io,
            format!("unexpected reply kind: {other:?}"),
        )),
    }
}

fn expect_written(reply: ReplyData) -> Result<u64, VDriveError> {
    match reply {
        ReplyData::Written { written } => Ok(written),
        other => Err(VDriveError::new(
            ErrorKind::Io,
            format!("unexpected reply kind: {other:?}"),
        )),
    }
}

#[async_trait]
impl FsBackend for VDriveClient {
    async fn statfs(&self) -> Result<StatFs, VDriveError> {
        let (reply, _) = self.rpc(Request::StatFs, None).await?;
        expect_statfs(reply)
    }

    async fn stat(&self, path: &str) -> Result<Entry, VDriveError> {
        let (reply, _) = self.rpc(Request::Stat { path: path.into() }, None).await?;
        expect_entry(reply)
    }

    async fn list(&self, path: &str) -> Result<Vec<Entry>, VDriveError> {
        let (reply, _) = self.rpc(Request::List { path: path.into() }, None).await?;
        expect_entries(reply)
    }

    async fn mkdir(&self, path: &str) -> Result<(), VDriveError> {
        self.rpc(Request::Mkdir { path: path.into() }, None).await?;
        Ok(())
    }

    async fn rmdir(&self, path: &str) -> Result<(), VDriveError> {
        self.rpc(Request::Rmdir { path: path.into() }, None).await?;
        Ok(())
    }

    async fn unlink(&self, path: &str) -> Result<(), VDriveError> {
        self.rpc(Request::Unlink { path: path.into() }, None).await?;
        Ok(())
    }

    async fn rename(&self, from: &str, to: &str) -> Result<(), VDriveError> {
        self.rpc(Request::Rename { from: from.into(), to: to.into() }, None)
            .await?;
        Ok(())
    }

    async fn truncate(&self, path: &str, size: u64) -> Result<(), VDriveError> {
        self.rpc(Request::Truncate { path: path.into(), size }, None)
            .await?;
        Ok(())
    }

    async fn create(&self, path: &str) -> Result<Entry, VDriveError> {
        let (reply, _) = self.rpc(Request::Create { path: path.into() }, None).await?;
        expect_entry(reply)
    }

    async fn read(&self, path: &str, offset: u64, len: u32) -> Result<Vec<u8>, VDriveError> {
        let len = len.min(MAX_CHUNK);
        let (reply, data) = self
            .rpc(Request::Read { path: path.into(), offset, len }, None)
            .await?;
        let _ = reply;
        Ok(data.unwrap_or_default())
    }

    async fn write(&self, path: &str, offset: u64, data: &[u8]) -> Result<u64, VDriveError> {
        if data.len() > MAX_CHUNK as usize {
            return Err(VDriveError::new(
                ErrorKind::TooLarge,
                format!("write chunk {} exceeds cap {MAX_CHUNK}", data.len()),
            ));
        }
        let (reply, _) = self
            .rpc(Request::Write { path: path.into(), offset }, Some(data))
            .await?;
        expect_written(reply)
    }
}

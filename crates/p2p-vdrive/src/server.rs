//! 服务端：/vdrive/fs/1 handler，一请求一流（无会话态，复用由底座连接池承担）。
//!
//! 安全面：仅受理已互认身份的入站流（裸流显式拒绝）；访问控制经
//! [AccessPolicy] 接缝按读写两类裁决；单请求数据帧上限 MAX_CHUNK。

use std::sync::Arc;

use async_trait::async_trait;
use p2p_identity::PeerId;
use p2p_mux::BoxedStream;
use p2p_protocol::{write_frame, ProtocolHandler, ProtocolId};
use tokio::io::AsyncWriteExt;

use crate::backend::FsBackend;
use crate::error::{ErrorKind, VDriveError};
use crate::reply::write_reply;
use crate::reply::ReplyData;
use crate::wire::{read_data_frame, read_frame_opt, Request, MAX_CHUNK, PROTO_FS};

/// 操作类别：读 / 写（策略裁决粒度）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpClass {
    Read,
    Write,
}

impl Request {
    pub fn op_class(&self) -> OpClass {
        match self {
            Request::Ping
            | Request::StatFs
            | Request::Stat { .. }
            | Request::List { .. }
            | Request::Read { .. } => OpClass::Read,
            Request::Mkdir { .. }
            | Request::Rmdir { .. }
            | Request::Unlink { .. }
            | Request::Rename { .. }
            | Request::Truncate { .. }
            | Request::Create { .. }
            | Request::Write { .. } => OpClass::Write,
        }
    }
}

/// 访问策略接缝：宿主可接 authz（capability key 登记随服务总控波）。
#[async_trait]
pub trait AccessPolicy: Send + Sync {
    async fn allow(&self, peer: &PeerId, class: OpClass) -> bool;
}

/// 默认全放行（身份已由传输层互认）。
pub struct AllowAll;

#[async_trait]
impl AccessPolicy for AllowAll {
    async fn allow(&self, _: &PeerId, _: OpClass) -> bool {
        true
    }
}

pub struct VDriveServer {
    proto: ProtocolId,
    backend: Arc<dyn FsBackend>,
    policy: Arc<dyn AccessPolicy>,
}

impl VDriveServer {
    pub fn new(backend: Arc<dyn FsBackend>) -> Result<Self, VDriveError> {
        Self::with_policy(backend, Arc::new(AllowAll))
    }

    pub fn with_policy(
        backend: Arc<dyn FsBackend>,
        policy: Arc<dyn AccessPolicy>,
    ) -> Result<Self, VDriveError> {
        Ok(Self {
            proto: ProtocolId::new(PROTO_FS)
                .map_err(|e| VDriveError::new(ErrorKind::Io, format!("protocol id: {e}")))?,
            backend,
            policy,
        })
    }
}

#[async_trait]
impl ProtocolHandler for VDriveServer {
    fn protocol(&self) -> ProtocolId {
        self.proto.clone()
    }

    async fn handle(&self, _stream: BoxedStream) -> std::io::Result<()> {
        Err(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "vdrive requires peer identity",
        ))
    }

    async fn handle_inbound(&self, peer: PeerId, mut stream: BoxedStream) -> std::io::Result<()> {
        let Some(frame) = read_frame_opt(&mut stream).await? else {
            return Ok(());
        };
        let req: Request = serde_json::from_slice(&frame).map_err(|e| {
            tracing::warn!(peer = %peer, "vdrive malformed request: {e}");
            std::io::Error::new(std::io::ErrorKind::InvalidData, "bad request frame")
        })?;
        if !self.policy.allow(&peer, req.op_class()).await {
            tracing::info!(peer = %peer, op = class_of(&req), "vdrive denied by policy");
            return write_reply(
                &mut stream,
                &Err(VDriveError::new(
                    ErrorKind::PermissionDenied,
                    "denied by policy",
                )),
            )
            .await;
        }
        match self.execute(req, &mut stream).await {
            Ok((reply, payload)) => {
                write_reply(&mut stream, &Ok(reply)).await?;
                if let Some(bytes) = payload {
                    // read 的数据帧在应答 JSON 之后（specs/vdrive.md §3）。
                    write_frame(&mut stream, &bytes).await?;
                }
                stream.flush().await
            }
            Err(e) => write_reply(&mut stream, &Err(e)).await,
        }
    }
}

fn class_of(req: &Request) -> &'static str {
    match req.op_class() {
        OpClass::Read => "read",
        OpClass::Write => "write",
    }
}

impl VDriveServer {
    /// 执行单请求；`payload` 为 read 成功时应答之后的数据帧。
    async fn execute(
        &self,
        req: Request,
        stream: &mut BoxedStream,
    ) -> Result<(ReplyData, Option<Vec<u8>>), VDriveError> {
        let backend = &self.backend;
        let payload = match req {
            Request::Ping => (ReplyData::Null, None),
            Request::StatFs => with_data(ReplyData::StatFs(backend.statfs().await?)),
            Request::Stat { path } => {
                with_data(ReplyData::Entry(Box::new(backend.stat(&path).await?)))
            }
            Request::List { path } => with_data(ReplyData::Entries(backend.list(&path).await?)),
            Request::Mkdir { path } => {
                backend.mkdir(&path).await?;
                (ReplyData::Null, None)
            }
            Request::Rmdir { path } => {
                backend.rmdir(&path).await?;
                (ReplyData::Null, None)
            }
            Request::Unlink { path } => {
                backend.unlink(&path).await?;
                (ReplyData::Null, None)
            }
            Request::Rename { from, to } => {
                backend.rename(&from, &to).await?;
                (ReplyData::Null, None)
            }
            Request::Truncate { path, size } => {
                backend.truncate(&path, size).await?;
                (ReplyData::Null, None)
            }
            Request::Create { path } => {
                with_data(ReplyData::Entry(Box::new(backend.create(&path).await?)))
            }
            Request::Read { path, offset, len } => {
                if len == 0 || len > MAX_CHUNK {
                    return Err(VDriveError::new(
                        ErrorKind::TooLarge,
                        format!("read len {len} exceeds chunk cap {MAX_CHUNK}"),
                    ));
                }
                let bytes = backend.read(&path, offset, len).await?;
                (ReplyData::Null, Some(bytes))
            }
            Request::Write { path, offset } => {
                let bytes = read_data_frame(stream).await.map_err(|e| {
                    VDriveError::new(ErrorKind::Io, format!("write data frame: {e}"))
                })?;
                let written = backend.write(&path, offset, &bytes).await?;
                with_data(ReplyData::Written { written })
            }
        };
        Ok(payload)
    }
}

fn with_data(reply: ReplyData) -> (ReplyData, Option<Vec<u8>>) {
    (reply, None)
}

/// 宿主装配入口：注册 handler 进节点。
pub fn serve(
    node: &p2p::Node,
    backend: Arc<dyn FsBackend>,
) -> Result<Arc<VDriveServer>, VDriveError> {
    serve_with_policy(node, backend, Arc::new(AllowAll))
}

pub fn serve_with_policy(
    node: &p2p::Node,
    backend: Arc<dyn FsBackend>,
    policy: Arc<dyn AccessPolicy>,
) -> Result<Arc<VDriveServer>, VDriveError> {
    let server = Arc::new(VDriveServer::with_policy(backend, policy)?);
    node.handle_protocol(server.clone());
    Ok(server)
}

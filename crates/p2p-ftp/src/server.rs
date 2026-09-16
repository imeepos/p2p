//! 服务端装配：控制 handler + 数据 handler。
//!
//! 传输令牌登记簿见 [crate::transfer]。控制会话签发令牌并等 oneshot
//! 结果；数据流入站兑付令牌、执行传输、回传结果。两 handler 均要求
//! 对端身份（swarm 安全握手互认），裸流一律显式拒绝。

use std::io;
use std::sync::Arc;
use std::time::Duration;

use p2p::Node;
use p2p_identity::PeerId;
use p2p_mux::BoxedStream;
use p2p_protocol::{read_frame, ProtocolHandler, ProtocolId};

use crate::auth::{AllowAll, Authenticator, Authorizer};
use crate::data_plane::run_data_transfer;
use crate::session;
use crate::transfer::TransferRegistry;
use crate::vfs::FileSystem;
use crate::wire::parse_data_header;
use crate::{proto, FtpError, PROTO_CTRL, PROTO_DATA};

/// 服务配置（超限即失败路径显式报错，禁止静默截断）。
#[derive(Debug, Clone)]
pub struct FtpConfig {
    /// 单次上传字节上限（STOR/APPE）。
    pub max_upload_bytes: u64,
    /// 传输令牌有效期（签发起算）。
    pub token_ttl: Duration,
    /// 控制侧等待数据通道完成的时限。
    pub transfer_timeout: Duration,
    /// 单次 LIST/NLST 条目数上限（防超大目录拖垮内存/带宽）。
    pub max_list_entries: u64,
}

impl Default for FtpConfig {
    fn default() -> Self {
        Self {
            max_upload_bytes: 256 << 20,
            token_ttl: Duration::from_secs(60),
            transfer_timeout: Duration::from_secs(300),
            max_list_entries: 10_000,
        }
    }
}

/// FTP 服务端：控制通道 handler。数据通道由 [`serve`] 一并装配。
pub struct FtpServer {
    proto: ProtocolId,
    fs: Arc<dyn FileSystem>,
    auth: Arc<dyn Authenticator>,
    authz: Arc<dyn Authorizer>,
    cfg: FtpConfig,
    transfers: TransferRegistry,
}

impl FtpServer {
    pub fn new(fs: Arc<dyn FileSystem>, auth: Arc<dyn Authenticator>) -> Result<Self, FtpError> {
        Self::with_config(fs, auth, FtpConfig::default())
    }

    pub fn with_config(
        fs: Arc<dyn FileSystem>,
        auth: Arc<dyn Authenticator>,
        cfg: FtpConfig,
    ) -> Result<Self, FtpError> {
        Self::with_parts(fs, auth, Arc::new(AllowAll), cfg)
    }

    /// 全量装配口：authz 逐命令授权（AllowAll = 既有语义零变化）。
    pub fn with_parts(
        fs: Arc<dyn FileSystem>,
        auth: Arc<dyn Authenticator>,
        authz: Arc<dyn Authorizer>,
        cfg: FtpConfig,
    ) -> Result<Self, FtpError> {
        let transfers = TransferRegistry::new(cfg.token_ttl);
        Ok(Self {
            proto: proto(PROTO_CTRL)?,
            fs,
            auth,
            authz,
            cfg,
            transfers,
        })
    }

    pub(crate) fn fs(&self) -> &Arc<dyn FileSystem> {
        &self.fs
    }

    pub(crate) fn auth(&self) -> &Arc<dyn Authenticator> {
        &self.auth
    }

    pub(crate) fn authz(&self) -> &Arc<dyn Authorizer> {
        &self.authz
    }

    pub(crate) fn cfg(&self) -> &FtpConfig {
        &self.cfg
    }

    pub(crate) fn transfers(&self) -> &TransferRegistry {
        &self.transfers
    }
}

#[async_trait::async_trait]
impl ProtocolHandler for FtpServer {
    fn protocol(&self) -> ProtocolId {
        self.proto.clone()
    }

    /// 裸流无对端身份，令牌无法绑定签发方：显式拒绝（禁静默服务）。
    async fn handle(&self, _stream: BoxedStream) -> io::Result<()> {
        Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "ftp control channel requires peer identity",
        ))
    }

    async fn handle_inbound(&self, peer: PeerId, stream: BoxedStream) -> io::Result<()> {
        session::run(self, peer, stream).await
    }
}

struct DataHandler {
    proto: ProtocolId,
    server: Arc<FtpServer>,
}

#[async_trait::async_trait]
impl ProtocolHandler for DataHandler {
    fn protocol(&self) -> ProtocolId {
        self.proto.clone()
    }

    async fn handle(&self, _stream: BoxedStream) -> io::Result<()> {
        Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "ftp data channel requires peer identity",
        ))
    }

    async fn handle_inbound(&self, peer: PeerId, mut stream: BoxedStream) -> io::Result<()> {
        let frame = read_frame(&mut stream).await?;
        let Some((_, token)) = parse_data_header(&frame) else {
            tracing::warn!(peer = %peer, "ftp data header malformed, closing");
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "bad data header",
            ));
        };
        let Some(pending) = self.server.transfers().take(&token, &peer) else {
            tracing::warn!(peer = %peer, "ftp data token unknown/expired/peer-mismatch, closing");
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "bad transfer token",
            ));
        };
        let outcome =
            run_data_transfer(&self.server, pending.kind, &pending.vpath, &mut stream).await;
        if pending.done.send(outcome).is_err() {
            tracing::warn!(path = %pending.vpath, "ftp control session gone before data result");
        }
        Ok(())
    }
}

/// 宿主装配入口：把控制/数据两个 handler 注册进节点，返回控制句柄。
pub fn serve(
    node: &Node,
    fs: Arc<dyn FileSystem>,
    auth: Arc<dyn Authenticator>,
) -> Result<Arc<FtpServer>, FtpError> {
    serve_with_config(node, fs, auth, FtpConfig::default())
}

pub fn serve_with_config(
    node: &Node,
    fs: Arc<dyn FileSystem>,
    auth: Arc<dyn Authenticator>,
    cfg: FtpConfig,
) -> Result<Arc<FtpServer>, FtpError> {
    serve_with_authz(node, fs, auth, Arc::new(AllowAll), cfg)
}

/// 全量装配口：带逐命令授权器（authz 收编用，FT6）。
pub fn serve_with_authz(
    node: &Node,
    fs: Arc<dyn FileSystem>,
    auth: Arc<dyn Authenticator>,
    authz: Arc<dyn Authorizer>,
    cfg: FtpConfig,
) -> Result<Arc<FtpServer>, FtpError> {
    let server = Arc::new(FtpServer::with_parts(fs, auth, authz, cfg)?);
    node.handle_protocol(server.clone());
    let data_proto = proto(PROTO_DATA)?;
    node.handle_protocol(Arc::new(DataHandler {
        proto: data_proto,
        server: server.clone(),
    }));
    Ok(server)
}

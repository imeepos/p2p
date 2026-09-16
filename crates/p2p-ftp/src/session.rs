//! 控制会话：命令解析 → 鉴权门 → 文件操作/传输编排 → 应答。
//!
//! 会话状态（登录态/虚拟 cwd/RNFR 暂存）仅存于本连接，连接结束即丢弃；
//! 虚拟路径一律经 [crate::vfs::normalize] 规范化，越出根回 550。

use std::io;

use p2p_identity::PeerId;
use p2p_mux::BoxedStream;
use p2p_protocol::read_frame;
use tokio::time::timeout;

use crate::server::FtpServer;
use crate::session_fs::{do_cwd, do_pass, do_remove, do_rnto, do_size};
use crate::transfer::DataKind;
use crate::vfs::{normalize, EntryKind};
use crate::wire::{hex, Command, Reply};

pub(crate) async fn run(
    server: &FtpServer,
    peer: PeerId,
    mut stream: BoxedStream,
) -> io::Result<()> {
    Reply::new(220, "p2p-ftp ready").write(&mut stream).await?;
    let mut st = SessionState::new();
    loop {
        let frame = read_frame(&mut stream).await?;
        let line = match String::from_utf8(frame) {
            Ok(l) => l,
            Err(_) => {
                Reply::new(501, "command must be utf-8")
                    .write(&mut stream)
                    .await?;
                continue;
            }
        };
        let cmd = Command::parse(&line);
        match dispatch(server, &mut st, &peer, cmd, &mut stream).await? {
            Flow::Continue => {}
            Flow::Quit => return Ok(()),
        }
    }
}

pub(crate) enum Flow {
    Continue,
    Quit,
}

pub(crate) struct SessionState {
    pub(crate) pending_user: Option<String>,
    pub(crate) user: Option<String>,
    pub(crate) cwd: String,
    pub(crate) rnfr: Option<String>,
}

impl SessionState {
    fn new() -> Self {
        Self {
            pending_user: None,
            user: None,
            cwd: "/".into(),
            rnfr: None,
        }
    }

    fn logged_in(&self) -> bool {
        self.user.is_some()
    }
}

pub(crate) async fn send(stream: &mut BoxedStream, reply: Reply) -> io::Result<()> {
    reply.write(stream).await
}

pub(crate) async fn reply(
    stream: &mut BoxedStream,
    code: u16,
    text: impl Into<String>,
) -> io::Result<()> {
    send(stream, Reply::new(code, text)).await
}

/// 文件系统错误 → 应答码映射：找不到/越狱 550，容量/策略超限 552，其余 451。
pub(crate) fn io_err_reply(e: &io::Error) -> Reply {
    let code = match e.kind() {
        io::ErrorKind::NotFound | io::ErrorKind::PermissionDenied => 550,
        io::ErrorKind::StorageFull | io::ErrorKind::InvalidData => 552,
        _ => 451,
    };
    Reply::new(code, e.to_string())
}

async fn dispatch(
    server: &FtpServer,
    st: &mut SessionState,
    peer: &PeerId,
    cmd: Command,
    stream: &mut BoxedStream,
) -> io::Result<Flow> {
    let pre_auth = matches!(
        cmd,
        Command::User(_)
            | Command::Pass(_)
            | Command::Quit
            | Command::Syst
            | Command::Feat
            | Command::Noop
            | Command::Unknown(_)
    );
    if !st.logged_in() && !pre_auth {
        return reply(stream, 530, "login required")
            .await
            .map(|()| Flow::Continue);
    }
    // 逐命令授权门（FT6）：命令分类 → Authorizer 判定，拒则 550。
    if let Some(op) = cmd.op() {
        let user = st.user.as_deref().unwrap_or("");
        if !server.authz().allow(peer, user, op) {
            tracing::warn!(peer = %peer, user = %user, op = ?op, "ftp command denied by authorizer");
            return reply(stream, 550, "permission denied")
                .await
                .map(|()| Flow::Continue);
        }
    }
    match cmd {
        Command::User(u) => {
            st.user = None;
            st.pending_user = Some(u);
            reply(stream, 331, "password required")
                .await
                .map(|()| Flow::Continue)
        }
        Command::Pass(p) => do_pass(server, st, peer, p, stream).await,
        Command::Quit => reply(stream, 221, "bye").await.map(|()| Flow::Quit),
        Command::Syst => reply(stream, 215, "UNIX Type: L8")
            .await
            .map(|()| Flow::Continue),
        Command::Feat => reply(stream, 211, "utf8 size list nlst retr stor appe")
            .await
            .map(|()| Flow::Continue),
        Command::Noop => reply(stream, 200, "ok").await.map(|()| Flow::Continue),
        Command::Type => reply(stream, 200, "binary only")
            .await
            .map(|()| Flow::Continue),
        Command::Pwd => reply(stream, 257, format!("\"{}\" is the cwd", st.cwd))
            .await
            .map(|()| Flow::Continue),
        Command::Cwd(p) => do_cwd(server, st, &p, stream).await,
        Command::Cdup => do_cwd(server, st, "..", stream).await,
        Command::Mkd(p) => match normalize(&st.cwd, &p) {
            Some(path) => match server.fs().mkdir(&path).await {
                Ok(()) => reply(stream, 257, format!("\"{path}\" created"))
                    .await
                    .map(|()| Flow::Continue),
                Err(e) => send(stream, io_err_reply(&e))
                    .await
                    .map(|()| Flow::Continue),
            },
            None => reject_path(stream).await,
        },
        Command::Rmd(p) => do_remove(server, st, &p, stream, true).await,
        Command::Dele(p) => do_remove(server, st, &p, stream, false).await,
        Command::Rnfr(p) => match normalize(&st.cwd, &p) {
            Some(path) => {
                st.rnfr = Some(path);
                reply(stream, 350, "ready for RNTO")
                    .await
                    .map(|()| Flow::Continue)
            }
            None => reject_path(stream).await,
        },
        Command::Rnto(p) => do_rnto(server, st, &p, stream).await,
        Command::Size(p) => do_size(server, st, &p, stream).await,
        Command::List(p) => {
            let arg = p.unwrap_or_else(|| st.cwd.clone());
            start_transfer(server, st, peer, DataKind::List, &arg, stream).await
        }
        Command::Nlst(p) => {
            let arg = p.unwrap_or_else(|| st.cwd.clone());
            start_transfer(server, st, peer, DataKind::Nlst, &arg, stream).await
        }
        Command::Retr(p) => start_transfer(server, st, peer, DataKind::Get, &p, stream).await,
        Command::Stor(p) => start_transfer(server, st, peer, DataKind::Put, &p, stream).await,
        Command::Appe(p) => start_transfer(server, st, peer, DataKind::Append, &p, stream).await,
        Command::Unknown(v) => reply(stream, 500, format!("unknown command {v}"))
            .await
            .map(|()| Flow::Continue),
    }
}

pub(crate) async fn reject_path(stream: &mut BoxedStream) -> io::Result<Flow> {
    reply(stream, 550, "path rejected")
        .await
        .map(|()| Flow::Continue)
}

/// 传输编排：预检 → 签发令牌 + 150 → 等数据通道结果 → 226/426。
async fn start_transfer(
    server: &FtpServer,
    st: &SessionState,
    peer: &PeerId,
    kind: DataKind,
    arg: &str,
    stream: &mut BoxedStream,
) -> io::Result<Flow> {
    let Some(path) = normalize(&st.cwd, arg) else {
        return reject_path(stream).await;
    };
    if let Some(r) = precheck(server, kind, &path).await {
        return send(stream, r).await.map(|()| Flow::Continue);
    }
    let (token, rx) = server.transfers().issue(*peer, kind, &path);
    Reply::new(150, format!("ok token={}", hex(&token)))
        .write(stream)
        .await?;
    let outcome = wait_outcome(server, rx, &path).await;
    let done = match outcome {
        Ok(n) => Reply::new(226, format!("ok n={n}")),
        Err(e) => io_err_reply(&e),
    };
    send(stream, done).await.map(|()| Flow::Continue)
}

async fn wait_outcome(
    server: &FtpServer,
    rx: tokio::sync::oneshot::Receiver<io::Result<u64>>,
    path: &str,
) -> io::Result<u64> {
    match timeout(server.cfg().transfer_timeout, rx).await {
        Ok(Ok(res)) => res,
        Ok(Err(_)) | Err(_) => {
            tracing::warn!(path = %path, "ftp data channel lost or timed out");
            Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "data channel lost or timed out",
            ))
        }
    }
}

/// 传输前置检查：不通过则直接回应答码，不签发令牌。
/// Put/APPE 目标允许不存在（新建语义），只查父目录；其余按目标检查。
async fn precheck(server: &FtpServer, kind: DataKind, path: &str) -> Option<Reply> {
    if matches!(kind, DataKind::Put | DataKind::Append) {
        let parent = parent_of(path);
        return match server.fs().metadata(&parent).await {
            Ok(p) if p.kind == EntryKind::Dir => None,
            Ok(_) => Some(Reply::new(550, "parent is not a directory")),
            Err(e) => Some(io_err_reply(&e)),
        };
    }
    let meta = match server.fs().metadata(path).await {
        Ok(m) => m,
        Err(e) => return Some(io_err_reply(&e)),
    };
    match kind {
        DataKind::Get if meta.kind == EntryKind::Dir => Some(Reply::new(550, "is a directory")),
        DataKind::List | DataKind::Nlst if meta.kind != EntryKind::Dir => {
            Some(Reply::new(550, "not a directory"))
        }
        _ => None,
    }
}

fn parent_of(path: &str) -> String {
    let trimmed = path.trim_end_matches('/');
    match trimmed.rfind('/') {
        Some(0) | None => "/".to_string(),
        Some(i) => trimmed[..i].to_string(),
    }
}

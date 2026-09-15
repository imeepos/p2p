//! 控制会话的文件操作命令实现（USER/PASS、CWD 族、删除/改名/查大小）。
//! 会话状态与应答助手归 [crate::session]，此处只消费。

use std::io;

use p2p_identity::PeerId;
use p2p_mux::BoxedStream;

use crate::server::FtpServer;
use crate::session::{io_err_reply, reject_path, reply, send, Flow, SessionState};
use crate::vfs::{normalize, EntryKind};

pub(crate) async fn do_pass(
    server: &FtpServer,
    st: &mut SessionState,
    peer: &PeerId,
    pass: String,
    stream: &mut BoxedStream,
) -> io::Result<Flow> {
    let Some(user) = st.pending_user.take() else {
        return reply(stream, 503, "USER first").await.map(|()| Flow::Continue);
    };
    if server.auth().login(peer, &user, &pass) {
        tracing::info!(peer = %peer, user = %user, "ftp login accepted");
        st.user = Some(user);
        reply(stream, 230, "logged in").await.map(|()| Flow::Continue)
    } else {
        tracing::warn!(peer = %peer, user = %user, "ftp login rejected");
        reply(stream, 530, "login incorrect").await.map(|()| Flow::Continue)
    }
}

pub(crate) async fn do_cwd(
    server: &FtpServer,
    st: &mut SessionState,
    arg: &str,
    stream: &mut BoxedStream,
) -> io::Result<Flow> {
    let Some(path) = normalize(&st.cwd, arg) else {
        return reject_path(stream).await;
    };
    match server.fs().metadata(&path).await {
        Ok(e) if e.kind == EntryKind::Dir => {
            st.cwd = path;
            reply(stream, 250, "cwd ok").await.map(|()| Flow::Continue)
        }
        Ok(_) => reply(stream, 550, "not a directory").await.map(|()| Flow::Continue),
        Err(e) => send(stream, io_err_reply(&e)).await.map(|()| Flow::Continue),
    }
}

pub(crate) async fn do_remove(
    server: &FtpServer,
    st: &SessionState,
    arg: &str,
    stream: &mut BoxedStream,
    is_dir: bool,
) -> io::Result<Flow> {
    let Some(path) = normalize(&st.cwd, arg) else {
        return reject_path(stream).await;
    };
    let result = if is_dir {
        server.fs().remove_dir(&path).await
    } else {
        server.fs().remove_file(&path).await
    };
    match result {
        Ok(()) => reply(stream, 250, "removed").await.map(|()| Flow::Continue),
        Err(e) => send(stream, io_err_reply(&e)).await.map(|()| Flow::Continue),
    }
}

pub(crate) async fn do_rnto(
    server: &FtpServer,
    st: &mut SessionState,
    arg: &str,
    stream: &mut BoxedStream,
) -> io::Result<Flow> {
    let Some(from) = st.rnfr.take() else {
        return reply(stream, 503, "RNFR first").await.map(|()| Flow::Continue);
    };
    let Some(to) = normalize(&st.cwd, arg) else {
        return reject_path(stream).await;
    };
    match server.fs().rename(&from, &to).await {
        Ok(()) => reply(stream, 250, "renamed").await.map(|()| Flow::Continue),
        Err(e) => send(stream, io_err_reply(&e)).await.map(|()| Flow::Continue),
    }
}

pub(crate) async fn do_size(
    server: &FtpServer,
    st: &SessionState,
    arg: &str,
    stream: &mut BoxedStream,
) -> io::Result<Flow> {
    let Some(path) = normalize(&st.cwd, arg) else {
        return reject_path(stream).await;
    };
    match server.fs().metadata(&path).await {
        Ok(e) if e.kind == EntryKind::File => {
            reply(stream, 213, e.size.to_string()).await.map(|()| Flow::Continue)
        }
        Ok(_) => reply(stream, 550, "not a regular file").await.map(|()| Flow::Continue),
        Err(e) => send(stream, io_err_reply(&e)).await.map(|()| Flow::Continue),
    }
}

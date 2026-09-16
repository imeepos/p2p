//! ftp 命令域：crates/p2p-ftp 客户端的一行式封装（一次性连接模式）。
//!
//! 每条命令独立装配最小节点（随机端口、mdns 关，同 chat context 惯例）→
//! 拨号对端 → 登录 → 单操作 → 退出；服务端装配归 daemon（serve.ftp 开关波）。
//! 退出码约定：成功 0；远端拒绝/网络失败经 CliError::Runtime → 1。

use std::path::PathBuf;
use std::sync::Arc;

use p2p::Node;
use p2p_ftp::{EntryKind, FtpClient, FtpError};
use tokio::io::AsyncWriteExt;

use crate::error::{CliError, CliResult};
use crate::node::DEFAULT_DATA_DIR;

/// 对端目标 + 身份根 + 登录账号（域内命令共通）。
#[derive(clap::Args)]
struct Target {
    /// 对端 PeerId（base58，服务端节点身份）
    peer: String,
    /// 登录账号（服务端 Authenticator 判定）
    #[arg(long, default_value = "anonymous")]
    user: String,
    /// 登录口令
    #[arg(long, default_value = "")]
    password: String,
    /// 身份数据目录
    #[arg(long, default_value = DEFAULT_DATA_DIR)]
    data_dir: String,
}

#[derive(clap::Args)]
struct LsArgs {
    #[command(flatten)]
    target: Target,
    /// 远端目录（缺省当前目录，登录后恒为根 /）
    path: Option<String>,
}

#[derive(clap::Args)]
struct GetArgs {
    #[command(flatten)]
    target: Target,
    /// 远端文件路径
    remote: String,
    /// 本地目标路径
    local: String,
}

#[derive(clap::Args)]
struct PutArgs {
    #[command(flatten)]
    target: Target,
    /// 本地源文件路径
    local: String,
    /// 远端目标路径
    remote: String,
}

#[derive(clap::Args)]
struct PathArgs {
    #[command(flatten)]
    target: Target,
    /// 远端路径
    path: String,
}

/// ftp 域注册：ls/get/put/mkdir/rmdir/delete/pwd（一次性连接，无 cd 状态）。
#[derive(clap::Subcommand)]
pub enum FtpCommand {
    /// 列远端目录（明细：类型/大小/mtime/名）
    Ls(LsArgs),
    /// 下载远端文件到本地路径
    Get(GetArgs),
    /// 上传本地文件到远端路径（服务端 HiddenStores：失败零残留）
    Put(PutArgs),
    /// 建远端目录
    Mkdir(PathArgs),
    /// 删远端空目录
    Rmdir(PathArgs),
    /// 删远端文件
    Delete(PathArgs),
    /// 查服务端当前目录（登录后恒为 /）
    Pwd(Target),
}

pub async fn run(command: FtpCommand) -> CliResult<()> {
    match command {
        FtpCommand::Ls(args) => ls(args).await,
        FtpCommand::Get(args) => get(args).await,
        FtpCommand::Put(args) => put(args).await,
        FtpCommand::Mkdir(args) => mkdir(args).await,
        FtpCommand::Rmdir(args) => rmdir(args).await,
        FtpCommand::Delete(args) => delete(args).await,
        FtpCommand::Pwd(target) => pwd(target).await,
    }
}

fn ftp_err(e: FtpError) -> CliError {
    CliError::Runtime(e.to_string())
}

fn parse_peer(s: &str) -> CliResult<p2p::PeerId> {
    let bytes =
        bs58::decode(s).into_vec().map_err(|e| CliError::Runtime(format!("peer 非法 base58: {e}")))?;
    let arr: [u8; 32] = bytes
        .try_into()
        .map_err(|_| CliError::Runtime("peer 长度须为 32 字节（base58 解码后）".into()))?;
    Ok(p2p::PeerId::from_bytes(arr))
}

/// 装配最小节点 → 拨号 → 登录；任一环失败即快速返回可读错误。
async fn dial(target: &Target) -> CliResult<FtpClient> {
    let node = Node::builder()
        .quic_port(0)
        .tcp_port(0)
        .mdns(false)
        .data_dir(PathBuf::from(&target.data_dir))
        .build()
        .await
        .map_err(|e| CliError::Runtime(format!("节点装配失败（data-dir={}）: {e}", target.data_dir)))?;
    let peer = parse_peer(&target.peer)?;
    let mut client = FtpClient::connect(Arc::new(node), peer)
        .await
        .map_err(ftp_err)?;
    client
        .login(&target.user, &target.password)
        .await
        .map_err(ftp_err)?;
    Ok(client)
}

fn fmt_entry(e: &p2p_ftp::Entry) -> String {
    let kind = match e.kind {
        EntryKind::Dir => 'd',
        EntryKind::File => 'f',
    };
    format!("{kind} {:>12} {} {}", e.size, e.mtime_unix, e.name)
}

async fn ls(args: LsArgs) -> CliResult<()> {
    let mut c = dial(&args.target).await?;
    let entries = c.list(args.path.as_deref()).await.map_err(ftp_err)?;
    for e in &entries {
        println!("{}", fmt_entry(e));
    }
    println!("共 {} 项 peer={}", entries.len(), args.target.peer);
    Ok(())
}

async fn get(args: GetArgs) -> CliResult<()> {
    let mut c = dial(&args.target).await?;
    let mut file = tokio::fs::File::create(&args.local)
        .await
        .map_err(|e| CliError::Runtime(format!("本地文件创建失败 {}: {e}", args.local)))?;
    let n = c.retr(&args.remote, &mut file).await.map_err(ftp_err)?;
    file.flush().await.map_err(|e| CliError::Runtime(format!("本地落盘失败: {e}")))?;
    println!("已下载 {} → {}（{n} 字节）", args.remote, args.local);
    Ok(())
}

async fn put(args: PutArgs) -> CliResult<()> {
    let mut c = dial(&args.target).await?;
    let mut file = tokio::fs::File::open(&args.local)
        .await
        .map_err(|e| CliError::Runtime(format!("本地文件打开失败 {}: {e}", args.local)))?;
    let n = c.stor(&args.remote, &mut file).await.map_err(ftp_err)?;
    println!("已上传 {} → {}（{n} 字节）", args.local, args.remote);
    Ok(())
}

async fn mkdir(args: PathArgs) -> CliResult<()> {
    let mut c = dial(&args.target).await?;
    c.mkd(&args.path).await.map_err(ftp_err)?;
    println!("已建目录 {} peer={}", args.path, args.target.peer);
    Ok(())
}

async fn rmdir(args: PathArgs) -> CliResult<()> {
    let mut c = dial(&args.target).await?;
    c.rmd(&args.path).await.map_err(ftp_err)?;
    println!("已删目录 {} peer={}", args.path, args.target.peer);
    Ok(())
}

async fn delete(args: PathArgs) -> CliResult<()> {
    let mut c = dial(&args.target).await?;
    c.dele(&args.path).await.map_err(ftp_err)?;
    println!("已删文件 {} peer={}", args.path, args.target.peer);
    Ok(())
}

async fn pwd(target: Target) -> CliResult<()> {
    let mut c = dial(&target).await?;
    let dir = c.pwd().await.map_err(ftp_err)?;
    println!("{dir} peer={}", target.peer);
    Ok(())
}

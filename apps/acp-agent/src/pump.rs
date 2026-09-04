//! 有界双向泵（设计 §4.2-3）：wire 侧 varint 帧 <-> 子进程侧 ndjson 行。
//! wire->child 用 acp-common LineReassembler（1 MiB 帧 / 16 MiB 行护栏），
//! child->wire 逐行有界读取；任何方向超限即 Err 断流，禁止无界缓冲。

use std::io::{self, ErrorKind};

use acp_common::chunk::{frames, LineReassembler};
use acp_common::consts::LINE_GUARD_LIMIT;
use acp_common::error::ErrorCode;
use p2p_protocol::{read_frame, write_frame};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{ChildStdin, ChildStdout};

/// acp-common 错误码 -> io 错误（swarm 只认 io::Result，msg 进日志）。
pub fn wire_error(code: ErrorCode) -> io::Error {
    io::Error::other(format!("wire ndjson violation: {code}"))
}

/// 读一条完整 ndjson 行（含换行）。握手期专用：调用方自行加超时。
pub async fn read_wire_line(
    stream: &mut (impl tokio::io::AsyncRead + Unpin + Send),
) -> io::Result<Vec<u8>> {
    let mut reassembler = LineReassembler::new();
    loop {
        if let Some(line) = reassembler.take_line() {
            return Ok(line);
        }
        let frame = read_frame(stream).await?;
        reassembler.push_frame(&frame).map_err(wire_error)?;
    }
}

/// 写一条 ndjson 行：acp-common 分块器切块，行尾换行以独立帧收尾。
pub async fn write_wire_line(
    stream: &mut (impl tokio::io::AsyncWrite + Unpin + Send),
    line: &[u8],
) -> io::Result<()> {
    for frame in frames(line) {
        write_frame(stream, frame).await?;
    }
    stream.flush().await
}

/// wire -> 子进程 stdin：帧重组为行原样直写。客户端半开/全关（EOF）=> Ok。
/// 客户端行超护栏 => Err（毒化），由会话层走退出阶梯。
pub async fn pump_wire_to_child(
    stream: &mut (impl tokio::io::AsyncRead + Unpin + Send),
    stdin: &mut ChildStdin,
) -> io::Result<()> {
    let mut reassembler = LineReassembler::new();
    loop {
        match read_frame(stream).await {
            Ok(frame) => {
                reassembler.push_frame(&frame).map_err(wire_error)?;
                while let Some(line) = reassembler.take_line() {
                    stdin.write_all(&line).await?;
                    stdin.flush().await?;
                }
            }
            Err(err) if err.kind() == ErrorKind::UnexpectedEof => return Ok(()),
            Err(err) => return Err(err),
        }
    }
}

/// 子进程 stdout -> wire：逐行有界读取后分块回写，行尾即 flush（背压不积压）。
/// 子进程退出（EOF）=> Ok；半行终止或行超护栏 => Err。
pub async fn pump_child_to_wire(
    stdout: &mut BufReader<ChildStdout>,
    wire: &mut (impl tokio::io::AsyncWrite + Unpin + Send),
) -> io::Result<()> {
    let mut line: Vec<u8> = Vec::new();
    loop {
        line.clear();
        let eof = read_bounded_line(stdout, &mut line).await?;
        if eof {
            return Ok(());
        }
        write_wire_line(wire, &line).await?;
    }
}

/// 读到一条不含换行的行；EOF 且无残留 => Ok(true)；EOF 带半行 => Err。
/// 会话泵与子进程读取任务（child 模块）共用。
pub(crate) async fn read_bounded_line(
    reader: &mut BufReader<ChildStdout>,
    out: &mut Vec<u8>,
) -> io::Result<bool> {
    out.clear();
    loop {
        let available = reader.fill_buf().await?;
        if available.is_empty() {
            if out.is_empty() {
                return Ok(true);
            }
            return Err(io::Error::other(format!(
                "child stdout ended with {} buffered bytes",
                out.len()
            )));
        }
        let (take, delim) = match available.iter().position(|&b| b == b'\n') {
            Some(pos) => (pos, 1usize),
            None => (available.len(), 0),
        };
        out.extend_from_slice(&available[..take]);
        reader.consume(take + delim);
        if out.len() > LINE_GUARD_LIMIT {
            return Err(line_guard_error(out.len()));
        }
        if delim == 1 {
            return Ok(false);
        }
    }
}

fn line_guard_error(len: usize) -> io::Error {
    io::Error::other(format!(
        "child line exceeds guard: {len} > {LINE_GUARD_LIMIT}"
    ))
}

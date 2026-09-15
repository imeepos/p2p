//! 控制通道与数据通道的线编码。
//!
//! 控制通道：一帧一行 UTF-8。命令 `CMD <arg>`，应答 `<3位码> <文本>`。
//! 数据通道首帧：1 字节操作码 + 32 字节一次性令牌；其后为原始字节流，
//! 方向由操作码决定（GET/LIST 服务端写，PUT 客户端写）。

use std::io;

use tokio::io::{AsyncRead, AsyncWrite};

use p2p_protocol::{read_frame, write_frame};

/// 数据操作码：RETR（服务端 → 客户端）。
pub const DATA_OP_GET: u8 = 1;
/// 数据操作码：STOR/APPE（客户端 → 服务端）。
pub const DATA_OP_PUT: u8 = 2;
/// 数据操作码：LIST 明细列表（服务端 → 客户端）。
pub const DATA_OP_LIST: u8 = 3;
/// 数据操作码：NLST 名字列表（服务端 → 客户端）。
pub const DATA_OP_NLST: u8 = 4;

/// 令牌字节数（256 位随机，抗猜测与重放）。
pub const TOKEN_LEN: usize = 32;

/// 数据通道首帧载荷。
pub fn data_header(op: u8, token: &[u8; TOKEN_LEN]) -> Vec<u8> {
    let mut frame = Vec::with_capacity(1 + TOKEN_LEN);
    frame.push(op);
    frame.extend_from_slice(token);
    frame
}

/// 解析数据通道首帧；长度不符或令牌缺字返回 None（由调用方断流）。
pub fn parse_data_header(frame: &[u8]) -> Option<(u8, [u8; TOKEN_LEN])> {
    let (&op, rest) = frame.split_first()?;
    if rest.len() != TOKEN_LEN {
        return None;
    }
    let mut token = [0u8; TOKEN_LEN];
    token.copy_from_slice(rest);
    Some((op, token))
}

/// 控制命令（RFC 959 子集）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    User(String),
    Pass(String),
    Syst,
    Feat,
    Noop,
    Quit,
    /// TYPE 恒为二进制语义，参数原样接受。
    Type,
    Pwd,
    Cwd(String),
    Cdup,
    Mkd(String),
    Rmd(String),
    Dele(String),
    Rnfr(String),
    Rnto(String),
    Size(String),
    /// None = 当前工作目录。
    List(Option<String>),
    Nlst(Option<String>),
    Retr(String),
    Stor(String),
    Appe(String),
    /// 无法识别的命令（服务端回 500）。
    Unknown(String),
}

impl Command {
    /// 宽松解析：语法错误不炸会话，折叠成 Unknown 由服务端统一回 500/501。
    pub fn parse(line: &str) -> Self {
        let line = line.trim();
        let (verb, arg) = match line.split_once(char::is_whitespace) {
            Some((v, a)) => (v, a.trim()),
            None => (line, ""),
        };
        let need = |a: &str, f: fn(String) -> Command| {
            if a.is_empty() {
                Command::Unknown(verb.to_string())
            } else {
                f(a.to_string())
            }
        };
        match verb.to_ascii_uppercase().as_str() {
            "USER" => need(arg, Command::User),
            "PASS" => Command::Pass(arg.to_string()),
            "SYST" => Command::Syst,
            "FEAT" => Command::Feat,
            "NOOP" => Command::Noop,
            "QUIT" => Command::Quit,
            "TYPE" => Command::Type,
            "PWD" => Command::Pwd,
            "CWD" => need(arg, Command::Cwd),
            "CDUP" => Command::Cdup,
            "MKD" => need(arg, Command::Mkd),
            "RMD" => need(arg, Command::Rmd),
            "DELE" => need(arg, Command::Dele),
            "RNFR" => need(arg, Command::Rnfr),
            "RNTO" => need(arg, Command::Rnto),
            "SIZE" => need(arg, Command::Size),
            "LIST" => Command::List(optional_arg(arg)),
            "NLST" => Command::Nlst(optional_arg(arg)),
            "RETR" => need(arg, Command::Retr),
            "STOR" => need(arg, Command::Stor),
            "APPE" => need(arg, Command::Appe),
            _ => Command::Unknown(verb.to_string()),
        }
    }

    /// 客户端编码为单行命令。
    pub fn encode(&self) -> String {
        match self {
            Command::User(u) => format!("USER {u}"),
            Command::Pass(p) => format!("PASS {p}"),
            Command::Syst => "SYST".into(),
            Command::Feat => "FEAT".into(),
            Command::Noop => "NOOP".into(),
            Command::Quit => "QUIT".into(),
            Command::Type => "TYPE I".into(),
            Command::Pwd => "PWD".into(),
            Command::Cwd(p) => format!("CWD {p}"),
            Command::Cdup => "CDUP".into(),
            Command::Mkd(p) => format!("MKD {p}"),
            Command::Rmd(p) => format!("RMD {p}"),
            Command::Dele(p) => format!("DELE {p}"),
            Command::Rnfr(p) => format!("RNFR {p}"),
            Command::Rnto(p) => format!("RNTO {p}"),
            Command::Size(p) => format!("SIZE {p}"),
            Command::List(p) => with_arg("LIST", p.as_deref()),
            Command::Nlst(p) => with_arg("NLST", p.as_deref()),
            Command::Retr(p) => format!("RETR {p}"),
            Command::Stor(p) => format!("STOR {p}"),
            Command::Appe(p) => format!("APPE {p}"),
            Command::Unknown(v) => v.clone(),
        }
    }
}

fn optional_arg(arg: &str) -> Option<String> {
    (!arg.is_empty()).then(|| arg.to_string())
}

fn with_arg(verb: &str, arg: Option<&str>) -> String {
    match arg {
        Some(p) => format!("{verb} {p}"),
        None => verb.to_string(),
    }
}

/// 服务端应答：`<3位码> <文本>` 单帧单条。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reply {
    pub code: u16,
    pub text: String,
}

impl Reply {
    pub fn new(code: u16, text: impl Into<String>) -> Self {
        Self {
            code,
            text: text.into(),
        }
    }

    pub fn parse(line: &str) -> Option<Self> {
        let line = line.trim_end();
        let (code, text) = line.split_once(' ')?;
        if code.len() != 3 || !code.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        Some(Self {
            code: code.parse().ok()?,
            text: text.to_string(),
        })
    }

    pub async fn write(&self, w: &mut (impl AsyncWrite + Unpin + Send)) -> io::Result<()> {
        write_frame(w, format!("{} {}", self.code, self.text).as_bytes()).await
    }

    pub async fn read(r: &mut (impl AsyncRead + Unpin + Send)) -> io::Result<Reply> {
        let frame = read_frame(r).await?;
        let line =
            String::from_utf8(frame).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        Reply::parse(&line).ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidData, format!("bad ftp reply: {line}"))
        })
    }

    /// 传输发起应答（150）中的令牌提取：文本形如 `ok token=<hex>`。
    pub fn transfer_token(&self) -> Option<[u8; TOKEN_LEN]> {
        let hex = self.text.split("token=").nth(1)?;
        unhex(hex.trim())
    }
}

pub fn hex(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push(char::from_digit(u32::from(b >> 4), 16).unwrap_or('0'));
        s.push(char::from_digit(u32::from(b & 0x0f), 16).unwrap_or('0'));
    }
    s
}

pub fn unhex(s: &str) -> Option<[u8; TOKEN_LEN]> {
    if s.len() != TOKEN_LEN * 2 || !s.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let mut out = [0u8; TOKEN_LEN];
    for (i, pair) in s.as_bytes().chunks(2).enumerate() {
        let hi = (pair[0] as char).to_digit(16)? as u8;
        let lo = (pair[1] as char).to_digit(16)? as u8;
        out[i] = (hi << 4) | lo;
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_parse_and_encode_roundtrip() {
        let cases = [
            ("user alice", Command::User("alice".into())),
            ("pass s3cret", Command::Pass("s3cret".into())),
            ("PWD", Command::Pwd),
            ("CWD /a b", Command::Cwd("/a b".into())),
            ("LIST", Command::List(None)),
            ("LIST /docs", Command::List(Some("/docs".into()))),
            ("retr x", Command::Retr("x".into())),
        ];
        for (line, want) in cases {
            let cmd = Command::parse(line);
            assert_eq!(Command::parse(&cmd.encode()), want, "roundtrip {line}");
        }
        assert_eq!(Command::parse("BOGUS x"), Command::Unknown("BOGUS".into()));
        assert_eq!(Command::parse("CWD"), Command::Unknown("CWD".into()));
    }

    #[test]
    fn reply_parse_and_token_roundtrip() {
        let r = Reply::parse("226 ok n=42").unwrap();
        assert_eq!(r.code, 226);
        assert!(r.code < 400);
        assert!(Reply::parse("2266 x").is_none());
        assert!(Reply::parse("garbage").is_none());

        let mut token = [0u8; TOKEN_LEN];
        token[0] = 0xab;
        token[31] = 0x07;
        let reply = Reply::new(150, format!("ok token={}", hex(&token)));
        assert_eq!(reply.transfer_token(), Some(token));
    }

    #[test]
    fn unhex_rejects_bad_input() {
        assert!(unhex("zz").is_none());
        assert!(unhex("abcd").is_none());
    }

    #[test]
    fn data_header_roundtrip() {
        let mut token = [1u8; TOKEN_LEN];
        token[7] = 9;
        let frame = data_header(DATA_OP_PUT, &token);
        assert_eq!(parse_data_header(&frame), Some((DATA_OP_PUT, token)));
        assert!(parse_data_header(&frame[..10]).is_none());
    }
}

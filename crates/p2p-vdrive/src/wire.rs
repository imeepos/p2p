//! 线格式：请求/应答编解码（specs/vdrive.md §2/§3）。
//!
//! 请求 = 一帧 JSON（`{"op":"...", ...}`）；写请求（write）后随一帧原始
//! 数据。应答 = 一帧 JSON（`{"ok":true,"data":...}` 或
//! `{"ok":false,"err":{"kind","msg"}}`）；读请求（read）成功时 data 为 null、
//! 后随一帧原始数据（≤ min(len, MAX_CHUNK)）。帧封装由底座
//! read_frame/write_frame 承担（varint 长度前缀，1 MiB 上限）。

use p2p_protocol::write_frame;
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use crate::error::VDriveError;

/// 文件系统操作协议 ID（registry.toml 已登记）。
pub const PROTO_FS: &str = "/vdrive/fs/1";
/// 单次 read/write 数据帧字节上限（帧上限 1 MiB 内留余量）。
pub const MAX_CHUNK: u32 = 512 * 1024;

/// 目录项元数据。mtime/ctime 为 unix 秒（0 = 后端未知）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entry {
    pub name: String,
    pub kind: EntryKind,
    pub size: u64,
    pub mtime: u64,
    pub ctime: u64,
}

/// 条目类别。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntryKind {
    File,
    Dir,
}

/// 文件系统容量（0 = 后端未知，映射为「不报告配额」）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct StatFs {
    pub total_bytes: u64,
    pub free_bytes: u64,
}

/// 请求：serde 内标签展开为 `{"op":"stat","path":"..."}` 形态。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum Request {
    Ping,
    StatFs,
    Stat { path: String },
    List { path: String },
    Mkdir { path: String },
    Rmdir { path: String },
    Unlink { path: String },
    Rename { from: String, to: String },
    Truncate { path: String, size: u64 },
    Create { path: String },
    Read { path: String, offset: u64, len: u32 },
    /// 写请求后必须随一帧原始数据（长度即写入量，≤ MAX_CHUNK）。
    Write { path: String, offset: u64 },
}

/// 成功应答 data 载荷（None → 线上 null）。
#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum ReplyData {    Entry(Box<Entry>),
    StatFs(StatFs),
    Entries(Vec<Entry>),
    Written { written: u64 },
    Null,
}

/// 应答帧编解码。
pub fn encode_reply(reply: &Result<ReplyData, VDriveError>) -> Vec<u8> {
    let value = match reply {
        Ok(data) => serde_json::json!({ "ok": true, "data": data }),
        Err(e) => serde_json::json!({ "ok": false, "err": e.to_wire() }),
    };
    serde_json::to_vec(&value).expect("reply json in-memory encode cannot fail")
}

/// 应答帧解码：ok=false 转 Err，形态非法报 InvalidData。
pub fn decode_reply(frame: &[u8]) -> Result<ReplyData, VDriveError> {
    let value: serde_json::Value = serde_json::from_slice(frame).map_err(|e| {
        VDriveError::new(
            crate::error::ErrorKind::Io,
            format!("reply not json: {e}"),
        )
    })?;
    match value.get("ok") {
        Some(serde_json::Value::Bool(true)) => parse_data(value.get("data")),
        Some(serde_json::Value::Bool(false)) => {
            let err = value
                .get("err")
                .cloned()
                .and_then(VDriveError::from_wire)
                .unwrap_or_else(|| VDriveError::new(crate::error::ErrorKind::Io, "bad error shape"));
            Err(err)
        }
        _ => Err(VDriveError::new(
            crate::error::ErrorKind::Io,
            "reply missing ok flag",
        )),
    }
}

fn parse_data(data: Option<&serde_json::Value>) -> Result<ReplyData, VDriveError> {
    let ctx = |e: serde_json::Error| {
        VDriveError::new(crate::error::ErrorKind::Io, format!("reply data shape: {e}"))
    };
    match data {
        None | Some(serde_json::Value::Null) => Ok(ReplyData::Null),
        Some(v) if v.get("total_bytes").is_some() => {
            Ok(ReplyData::StatFs(serde_json::from_value(v.clone()).map_err(ctx)?))
        }
        Some(v) if v.is_array() => {
            Ok(ReplyData::Entries(serde_json::from_value(v.clone()).map_err(ctx)?))
        }
        Some(v) if v.get("written").is_some() => {
            Ok(ReplyData::Written { written: v["written"].as_u64().unwrap_or(0) })
        }
        Some(v) => Ok(ReplyData::Entry(Box::new(serde_json::from_value(v.clone()).map_err(ctx)?))),
    }
}

/// 请求帧读写：一帧 JSON；`needs_data` 的请求（Write）后随一帧数据。
pub async fn write_request(
    w: &mut (impl AsyncWrite + Unpin + Send),
    req: &Request,
    data: Option<&[u8]>,
) -> std::io::Result<()> {
    let json = serde_json::to_vec(req).map_err(|e| {
        std::io::Error::new(std::io::ErrorKind::InvalidData, format!("encode request: {e}"))
    })?;
    write_frame(w, &json).await?;
    if let Some(bytes) = data {
        write_frame(w, bytes).await?;
    }
    w.flush().await
}

/// 请求帧读入（数据帧由调用方按 op 另读）。
pub async fn read_request(
    r: &mut (impl AsyncRead + Unpin + Send),
) -> std::io::Result<Option<Request>> {
    let Some(frame) = read_frame_opt(r).await? else {
        return Ok(None);
    };
    let req: Request = serde_json::from_slice(&frame).map_err(|e| {
        std::io::Error::new(std::io::ErrorKind::InvalidData, format!("bad request: {e}"))
    })?;
    Ok(Some(req))
}

/// 应答帧写出。
pub async fn write_reply(
    w: &mut (impl AsyncWrite + Unpin + Send),
    reply: &Result<ReplyData, VDriveError>,
) -> std::io::Result<()> {
    write_frame(w, &encode_reply(reply)).await?;
    w.flush().await
}

/// 读数据帧（write 请求数据 / read 应答数据），EOF 报 UnexpectedEof。
pub async fn read_data_frame(r: &mut (impl AsyncRead + Unpin + Send)) -> std::io::Result<Vec<u8>> {
    read_frame_opt(r)
        .await?
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::UnexpectedEof, "missing data frame"))
}

/// 会话循环用帧读取：流干净结束（EOF 在帧首）返回 None；其余语义同底座
/// read_frame（varint 长度前缀 + 1 MiB 上限）。
pub async fn read_frame_opt(
    r: &mut (impl AsyncRead + Unpin + Send),
) -> std::io::Result<Option<Vec<u8>>> {
    let mut first = [0u8; 1];
    if r.read(&mut first).await? == 0 {
        return Ok(None);
    }
    let mut len = u64::from(first[0] & 0x7f);
    let mut shift: u32 = 0;
    let mut cont = first[0] & 0x80 != 0;
    while cont {
        let mut byte = [0u8; 1];
        r.read_exact(&mut byte).await?;
        len |= u64::from(byte[0] & 0x7f) << (shift + 7);
        shift += 7;
        cont = byte[0] & 0x80 != 0;
    }
    if len > p2p_protocol::MAX_FRAME_SIZE as u64 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("frame too large: {len}"),
        ));
    }
    let mut buf = vec![0u8; len as usize];
    r.read_exact(&mut buf).await?;
    Ok(Some(buf))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn roundtrip(req: Request, json_has: &[&str]) {
        let json = serde_json::to_vec(&req).unwrap();
        let s = String::from_utf8(json).unwrap();
        for frag in json_has {
            assert!(s.contains(frag), "{s} missing {frag}");
        }
        assert_eq!(serde_json::from_slice::<Request>(s.as_bytes()).unwrap(), req);
    }

    #[test]
    fn request_json_shapes() {
        roundtrip(Request::Ping, &[r#""op":"ping""#]);
        roundtrip(Request::Stat { path: "/a".into() }, &[r#""op":"stat""#, r#""path":"/a""#]);
        roundtrip(
            Request::Read { path: "/a".into(), offset: 8, len: 64 },
            &[r#""op":"read""#, r#""offset":8"#, r#""len":64"#],
        );
        roundtrip(
            Request::Rename { from: "/a".into(), to: "/b".into() },
            &[r#""op":"rename""#],
        );
    }

    #[test]
    fn reply_ok_shapes() {
        let e = Entry {
            name: "a.txt".into(),
            kind: EntryKind::File,
            size: 3,
            mtime: 9,
            ctime: 9,
        };
        let frame = encode_reply(&Ok(ReplyData::Entry(Box::new(e.clone()))));
        let s = String::from_utf8(frame).unwrap();
        // serde_json 键序不保证；只断言语义字段（ok + data 形态）。
        assert!(s.contains(r#""ok":true"#) && s.contains(r#""data""#), "{s}");
        assert!(matches!(decode_reply(s.as_bytes()).unwrap(), ReplyData::Entry(back) if *back == e));

        let frame = encode_reply(&Ok(ReplyData::Written { written: 5 }));
        assert!(matches!(decode_reply(&frame).unwrap(), ReplyData::Written { written: 5 }));

        assert!(matches!(decode_reply(&encode_reply(&Ok(ReplyData::Null))).unwrap(), ReplyData::Null));
        let list = encode_reply(&Ok(ReplyData::Entries(vec![e])));
        assert!(matches!(decode_reply(&list).unwrap(), ReplyData::Entries(v) if v.len() == 1));
    }

    #[test]
    fn reply_err_shape() {
        let err = VDriveError::new(crate::error::ErrorKind::NotFound, "gone");
        let frame = encode_reply(&Err(err.clone()));
        let s = String::from_utf8(frame.clone()).unwrap();
        assert!(s.contains(r#""ok":false"#) && s.contains(r#""kind":"not_found""#), "{s}");
        assert_eq!(decode_reply(&frame).unwrap_err(), err);
    }
}

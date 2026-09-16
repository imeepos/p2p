//! 应答帧编解码（specs/vdrive.md §2.2）。
//!
//! 成功：`{"ok":true,"data":<载荷>}`；失败：`{"ok":false,"err":{"kind","msg"}}`。

use tokio::io::{AsyncWrite, AsyncWriteExt};

use crate::error::{ErrorKind, VDriveError};
use crate::wire::{Entry, StatFs};
use p2p_protocol::write_frame;

/// 成功应答 data 载荷（None → 线上 null）。
#[derive(Debug, Clone, serde::Serialize)]
#[serde(untagged)]
pub enum ReplyData {
    Entry(Box<Entry>),
    StatFs(StatFs),
    Entries(Vec<Entry>),
    Written { written: u64 },
    Null,
}

/// 应答帧编解码。
pub fn encode_reply(reply: &Result<ReplyData, VDriveError>) -> std::io::Result<Vec<u8>> {
    let value = match reply {
        Ok(data) => serde_json::json!({ "ok": true, "data": data }),
        Err(e) => serde_json::json!({ "ok": false, "err": e.to_wire() }),
    };
    serde_json::to_vec(&value).map_err(|e| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("encode reply: {e}"),
        )
    })
}

/// 应答帧解码：ok=false 转 Err，形态非法报 InvalidData。
pub fn decode_reply(frame: &[u8]) -> Result<ReplyData, VDriveError> {
    let value: serde_json::Value = serde_json::from_slice(frame)
        .map_err(|e| VDriveError::new(ErrorKind::Io, format!("reply not json: {e}")))?;
    match value.get("ok") {
        Some(serde_json::Value::Bool(true)) => parse_data(value.get("data")),
        Some(serde_json::Value::Bool(false)) => {
            let err = value
                .get("err")
                .cloned()
                .and_then(VDriveError::from_wire)
                .unwrap_or_else(|| VDriveError::new(ErrorKind::Io, "bad error shape"));
            Err(err)
        }
        _ => Err(VDriveError::new(ErrorKind::Io, "reply missing ok flag")),
    }
}

fn parse_data(data: Option<&serde_json::Value>) -> Result<ReplyData, VDriveError> {
    let ctx =
        |e: serde_json::Error| VDriveError::new(ErrorKind::Io, format!("reply data shape: {e}"));
    match data {
        None | Some(serde_json::Value::Null) => Ok(ReplyData::Null),
        Some(v) if v.get("total_bytes").is_some() => Ok(ReplyData::StatFs(
            serde_json::from_value(v.clone()).map_err(ctx)?,
        )),
        Some(v) if v.is_array() => Ok(ReplyData::Entries(
            serde_json::from_value(v.clone()).map_err(ctx)?,
        )),
        Some(v) if v.get("written").is_some() => Ok(ReplyData::Written {
            written: v["written"].as_u64().unwrap_or(0),
        }),
        Some(v) => Ok(ReplyData::Entry(Box::new(
            serde_json::from_value(v.clone()).map_err(ctx)?,
        ))),
    }
}

/// 应答帧写出。
pub async fn write_reply(
    w: &mut (impl AsyncWrite + Unpin + Send),
    reply: &Result<ReplyData, VDriveError>,
) -> std::io::Result<()> {
    let bytes = encode_reply(reply)?;
    write_frame(w, &bytes).await?;
    w.flush().await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Entry {
        Entry {
            name: "a.txt".into(),
            kind: crate::wire::EntryKind::File,
            size: 3,
            mtime: 9,
            ctime: 9,
        }
    }

    #[test]
    fn reply_ok_shapes() {
        let e = sample();
        let frame = encode_reply(&Ok(ReplyData::Entry(Box::new(e.clone())))).unwrap();
        let s = String::from_utf8(frame).unwrap();
        // serde_json 键序不保证；只断言语义字段（ok + data 形态）。
        assert!(s.contains(r#""ok":true"#) && s.contains(r#""data""#), "{s}");
        assert!(
            matches!(decode_reply(s.as_bytes()).unwrap(), ReplyData::Entry(back) if *back == e)
        );

        let frame = encode_reply(&Ok(ReplyData::Written { written: 5 })).unwrap();
        assert!(matches!(
            decode_reply(&frame).unwrap(),
            ReplyData::Written { written: 5 }
        ));

        assert!(matches!(
            decode_reply(&encode_reply(&Ok(ReplyData::Null)).unwrap()).unwrap(),
            ReplyData::Null
        ));
        let list = encode_reply(&Ok(ReplyData::Entries(vec![e]))).unwrap();
        assert!(matches!(decode_reply(&list).unwrap(), ReplyData::Entries(v) if v.len() == 1));
    }

    #[test]
    fn reply_err_shape() {
        let err = VDriveError::new(ErrorKind::NotFound, "gone");
        let frame = encode_reply(&Err(err.clone())).unwrap();
        let s = String::from_utf8(frame.clone()).unwrap();
        assert!(
            s.contains(r#""ok":false"#) && s.contains(r#""kind":"not_found""#),
            "{s}"
        );
        assert_eq!(decode_reply(&frame).unwrap_err(), err);
    }

    #[test]
    fn reply_missing_ok_flag_rejected() {
        let err = decode_reply(br#"{"data":null}"#).unwrap_err();
        assert_eq!(err.kind, ErrorKind::Io);
        let err = decode_reply(b"not-json").unwrap_err();
        assert_eq!(err.kind, ErrorKind::Io);
    }
}

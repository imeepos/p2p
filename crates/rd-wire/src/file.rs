//! /rd/file/1 文件传输消息（remote-desktop-plan §3.3）。
//! JSON 控制 + base64 数据块；解码端对路径/大小越界一律拒绝。

use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use serde::{Deserialize, Serialize};

use crate::WireError;

/// 单块原始数据上限（384 KiB → base64 ≈ 512 KiB ≤ 1 MiB 帧上限）。
pub const MAX_DATA_RAW_BYTES: usize = 384 * 1024;
/// 路径字符上限。
pub const MAX_PATH_CHARS: usize = 4096;

/// 条目类型。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FsKind {
    File,
    Dir,
    Other,
}

/// 目录条目。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entry {
    pub name: String,
    pub kind: FsKind,
    pub size: u64,
    pub mtime: u64,
}

/// 传输方向（相对 viewer）：Upload = viewer→host。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum XferDir {
    Upload,
    Download,
}

/// 文件通道消息全集（v1）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum FileMsg {
    /// 目录浏览请求（viewer→host）。
    FsList { path: String },
    /// 目录条目应答（error 非空 = 浏览失败）。
    FsListAck {
        path: String,
        entries: Vec<Entry>,
        error: Option<String>,
    },
    /// 单条目 stat。
    FsStat { path: String },
    /// stat 应答（entry=None 表示不存在；error 非空 = stat 失败）。
    FsStatAck {
        path: String,
        entry: Option<Entry>,
        error: Option<String>,
    },
    /// 建目录。
    FsMkdir { path: String },
    /// 删除（host 实现选择移除或回收站语义）。
    FsRm { path: String },
    /// 建目录/删除应答（M5 加法）。
    FsOpAck { ok: bool, reason: Option<String> },
    /// 传输登记（viewer 生成 id；download = host 读文件发回）。
    XferStart {
        id: String,
        path: String,
        size: u64,
        direction: XferDir,
    },
    /// 接受/拒绝；accept 可带续传 offset（host 侧已有 n 字节）。
    XferAck {
        id: String,
        ok: bool,
        reason: Option<String>,
        offset: u64,
    },
    /// 分块数据（base64，≤ MAX_DATA_RAW_BYTES 原始字节）。
    XferData {
        id: String,
        offset: u64,
        data: String,
    },
    /// 完成。
    XferEnd { id: String, ok: bool },
    /// 任意时刻单方取消。
    XferAbort { id: String },
    /// 进度广播（done/total 字节）。
    XferProgress { id: String, done: u64, total: u64 },
}

impl FileMsg {
    /// 编码（含路径卫生与数据大小校验）。
    pub fn encode(&self) -> Result<Vec<u8>, WireError> {
        if let FileMsg::XferData { data, .. } = self {
            let raw = B64
                .decode(data)
                .map_err(|e| WireError::Invalid(format!("xfer data not valid base64: {e}")))?;
            if raw.len() > MAX_DATA_RAW_BYTES {
                return Err(WireError::Invalid("xfer data chunk too large".into()));
            }
        }
        validate_paths(self)?;
        Ok(serde_json::to_vec(self)?)
    }

    /// 解码 + 校验。
    pub fn decode(bytes: &[u8]) -> Result<Self, WireError> {
        let msg: FileMsg = serde_json::from_slice(bytes)?;
        msg.encode().map(|_| msg)
    }

    /// 构造数据块（原始字节 → base64），超限即错。
    pub fn xfer_data(id: String, offset: u64, raw: &[u8]) -> Result<Self, WireError> {
        if raw.len() > MAX_DATA_RAW_BYTES {
            return Err(WireError::Invalid("xfer data chunk too large".into()));
        }
        Ok(FileMsg::XferData {
            id,
            offset,
            data: B64.encode(raw),
        })
    }

    /// 取回数据块原始字节（无效 base64 即错）。
    pub fn xfer_data_raw(&self) -> Result<Vec<u8>, WireError> {
        let FileMsg::XferData { data, .. } = self else {
            return Err(WireError::Invalid("not a XferData message".into()));
        };
        B64.decode(data)
            .map_err(|e| WireError::Invalid(format!("xfer data not valid base64: {e}")))
    }
}

/// 路径字段全部走相对 POSIX 路径卫生（禁绝对/`..`/NUL/超长）。
/// 例外：FsList/FsStat 的空串表示根目录浏览（M5 加法），其余消息空串仍拒绝。
fn validate_paths(msg: &FileMsg) -> Result<(), WireError> {
    let paths: Vec<(&str, bool)> = match msg {
        FileMsg::FsList { path } | FileMsg::FsStat { path } => vec![(path, true)],
        FileMsg::FsMkdir { path } | FileMsg::FsRm { path } | FileMsg::XferStart { path, .. } => {
            vec![(path, false)]
        }
        _ => Vec::new(),
    };
    for (p, allow_root) in paths {
        if allow_root && p.is_empty() {
            continue;
        }
        sanitize_rel_path(p)?;
    }
    Ok(())
}

/// 相对 POSIX 路径卫生：非空、非绝对、无 `..` 段、无 NUL、≤ MAX_PATH_CHARS。
/// rd-fs 落盘前 MUST 以本函数结果为准（防路径穿越）。
pub fn sanitize_rel_path(path: &str) -> Result<String, WireError> {
    if path.is_empty() {
        return Err(WireError::Invalid("empty path".into()));
    }
    if path.len() > MAX_PATH_CHARS {
        return Err(WireError::Invalid("path too long".into()));
    }
    if path.starts_with('/') || path.contains('\0') {
        return Err(WireError::Invalid(format!("unsafe path: {path}")));
    }
    for seg in path.split('/') {
        if seg.is_empty() || seg == "." || seg == ".." {
            return Err(WireError::Invalid(format!("unsafe path: {path}")));
        }
    }
    Ok(path.to_string())
}

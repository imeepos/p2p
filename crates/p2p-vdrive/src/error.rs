//! 错误模型：线上的结构化错误（kind + msg）与本地的 io::Error 互转。
//!
//! kind 是协议契约（specs/vdrive.md §4），两端按 kind 语义翻译；
//! msg 仅人类可读，不参与判定。

use std::io;

/// 结构化错误类别（线上以 snake_case 字符串传输）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorKind {
    NotFound,
    AlreadyExists,
    NotEmpty,
    NotDir,
    PermissionDenied,
    InvalidPath,
    TooLarge,
    Unsupported,
    Io,
}

impl ErrorKind {
    pub fn as_str(self) -> &'static str {
        match self {
            ErrorKind::NotFound => "not_found",
            ErrorKind::AlreadyExists => "already_exists",
            ErrorKind::NotEmpty => "not_empty",
            ErrorKind::NotDir => "not_dir",
            ErrorKind::PermissionDenied => "permission_denied",
            ErrorKind::InvalidPath => "invalid_path",
            ErrorKind::TooLarge => "too_large",
            ErrorKind::Unsupported => "unsupported",
            ErrorKind::Io => "io",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "not_found" => ErrorKind::NotFound,
            "already_exists" => ErrorKind::AlreadyExists,
            "not_empty" => ErrorKind::NotEmpty,
            "not_dir" => ErrorKind::NotDir,
            "permission_denied" => ErrorKind::PermissionDenied,
            "invalid_path" => ErrorKind::InvalidPath,
            "too_large" => ErrorKind::TooLarge,
            "unsupported" => ErrorKind::Unsupported,
            "io" => ErrorKind::Io,
            _ => return None,
        })
    }
}

/// 协议错误：kind 语义化，msg 可读。
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{kind:?}: {msg}")]
pub struct VDriveError {
    pub kind: ErrorKind,
    pub msg: String,
}

impl VDriveError {
    pub fn new(kind: ErrorKind, msg: impl Into<String>) -> Self {
        Self {
            kind,
            msg: msg.into(),
        }
    }

    /// 线上 JSON 形态：{"kind": "...", "msg": "..."}。
    pub fn to_wire(&self) -> serde_json::Value {
        serde_json::json!({ "kind": self.kind.as_str(), "msg": self.msg })
    }

    pub fn from_wire(value: serde_json::Value) -> Option<Self> {
        let kind = ErrorKind::parse(value.get("kind")?.as_str()?)?;
        let msg = value
            .get("msg")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("")
            .to_string();
        Some(Self { kind, msg })
    }
}

impl From<io::Error> for VDriveError {
    fn from(e: io::Error) -> Self {
        let kind = match e.kind() {
            io::ErrorKind::NotFound => ErrorKind::NotFound,
            io::ErrorKind::AlreadyExists => ErrorKind::AlreadyExists,
            io::ErrorKind::DirectoryNotEmpty => ErrorKind::NotEmpty,
            io::ErrorKind::NotADirectory => ErrorKind::NotDir,
            io::ErrorKind::PermissionDenied => ErrorKind::PermissionDenied,
            io::ErrorKind::InvalidInput | io::ErrorKind::InvalidData => ErrorKind::InvalidPath,
            _ => ErrorKind::Io,
        };
        Self::new(kind, e.to_string())
    }
}

/// 转本地 io::Error（保留 ErrorKind 映射，供通用 io 侧消费）。
impl From<VDriveError> for io::Error {
    fn from(e: VDriveError) -> io::Error {
        let kind = match e.kind {
            ErrorKind::NotFound => io::ErrorKind::NotFound,
            ErrorKind::AlreadyExists => io::ErrorKind::AlreadyExists,
            ErrorKind::NotEmpty => io::ErrorKind::DirectoryNotEmpty,
            ErrorKind::NotDir => io::ErrorKind::NotADirectory,
            ErrorKind::PermissionDenied => io::ErrorKind::PermissionDenied,
            ErrorKind::InvalidPath => io::ErrorKind::InvalidInput,
            ErrorKind::TooLarge | ErrorKind::Unsupported | ErrorKind::Io => io::ErrorKind::Other,
        };
        io::Error::new(kind, format!("{:?}: {}", e.kind, e.msg))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kind_wire_roundtrip() {
        for kind in [
            ErrorKind::NotFound,
            ErrorKind::AlreadyExists,
            ErrorKind::NotEmpty,
            ErrorKind::NotDir,
            ErrorKind::PermissionDenied,
            ErrorKind::InvalidPath,
            ErrorKind::TooLarge,
            ErrorKind::Unsupported,
            ErrorKind::Io,
        ] {
            assert_eq!(ErrorKind::parse(kind.as_str()), Some(kind));
        }
        assert_eq!(ErrorKind::parse("bogus"), None);
    }

    #[test]
    fn error_wire_roundtrip() {
        let e = VDriveError::new(ErrorKind::NotFound, "no such file");
        let back = VDriveError::from_wire(e.to_wire()).unwrap();
        assert_eq!(back.kind, ErrorKind::NotFound);
        assert_eq!(back.msg, "no such file");
        assert!(VDriveError::from_wire(serde_json::json!({"kind": "?"})).is_none());
    }

    #[test]
    fn io_error_kind_mapping() {
        let e: VDriveError = io::Error::from(io::ErrorKind::NotFound).into();
        assert_eq!(e.kind, ErrorKind::NotFound);
    }
}

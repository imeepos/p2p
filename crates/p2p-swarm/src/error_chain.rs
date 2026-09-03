//! E9-T3 错误链保真：`other(e.to_string())` 写法把内层错误拍平成纯文本，
//! source() 遍历就此终止（E9-Q0 §3.2；先例：p2p-mux ChainedPayload，E7-K2）。
//! 本载荷 Display 透传内层原文，source() 即内层错误，调用方可沿链 downcast 还原。

use std::error::Error;
use std::io;

/// 错误链载荷：Display 保持内层原文，source() 返回内层。
#[derive(Debug)]
struct ChainedError<E> {
    inner: E,
}

impl<E: std::fmt::Display> std::fmt::Display for ChainedError<E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.inner.fmt(f)
    }
}

impl<E: Error + 'static> Error for ChainedError<E> {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(&self.inner)
    }
}

/// 以 Other 类别包装内层错误，替代 to_string 拍平写法：
/// to_string() 与旧实现逐字一致，额外保住 source() 链。
pub(crate) fn chained<E>(e: E) -> io::Error
where
    E: Error + Send + Sync + 'static,
{
    io::Error::other(ChainedError { inner: e })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 保真核心：source 可还原内层类型，Display 与内层逐字一致。
    #[test]
    fn chained_keeps_source_and_display() {
        let err = chained(io::Error::new(
            io::ErrorKind::ConnectionRefused,
            "mock refused",
        ));
        assert_eq!(err.to_string(), "mock refused");
        let src = err.source().expect("source must survive wrapping");
        let src = src
            .downcast_ref::<io::Error>()
            .expect("inner io::Error reachable");
        assert_eq!(src.kind(), io::ErrorKind::ConnectionRefused);
    }

    /// 消融基线：to_string 拍平正是丢 source，钉住事实防回退混淆。
    #[test]
    fn to_string_flattening_drops_source() {
        let err = io::Error::other("flattened".to_string());
        assert!(err.source().is_none(), "string payload has no source");
    }
}

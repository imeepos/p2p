//! 退出码约定：0 成功（正常返回）/ 1 运行失败（[CliError::Runtime]）/
//! 2 用法错误（clap 用法错误自动以 2 退出，不经本模块）。

use std::fmt;

/// 运行失败。
pub const EXIT_RUNTIME: i32 = 1;

#[derive(Debug)]
pub enum CliError {
    /// 运行期失败（探测 IO、节点交互等），退出码 1。
    Runtime(String),
}

impl CliError {
    pub fn exit_code(&self) -> i32 {
        match self {
            CliError::Runtime(_) => EXIT_RUNTIME,
        }
    }
}

impl fmt::Display for CliError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CliError::Runtime(m) => write!(f, "运行失败: {m}"),
        }
    }
}

pub type CliResult<T> = Result<T, CliError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_error_maps_to_exit_one() {
        assert_eq!(EXIT_RUNTIME, 1);
        assert_eq!(CliError::Runtime("io".into()).exit_code(), EXIT_RUNTIME);
        assert!(CliError::Runtime("io".into())
            .to_string()
            .starts_with("运行失败"));
    }
}

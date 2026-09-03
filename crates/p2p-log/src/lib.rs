//! p2p-log：统一日志设施（E7-L1）。
//!
//! 职责：RUST_LOG 未设置时默认 info 级别；文本与 JSON 两种输出格式；
//! 可选滚动文件落盘（大小与份数双上限封顶，见 [rolling]）；重复初始化幂等；
//! panic 钩子把 panic 信息写入日志并回显 stderr（见 [panic]）。
//!
//! 失败路径可观测红线：日志落盘初始化失败不静默——回退 stderr 输出并留告警
//! （[InitReport::fallback] + eprintln）。[init] 不返回 Err，结果一律通过
//! [InitReport] 表达。

use std::path::PathBuf;
use std::sync::OnceLock;

use tracing_subscriber::filter::LevelFilter;
use tracing_subscriber::fmt;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::EnvFilter;

pub mod panic;
pub mod paths;
pub mod rolling;

pub use paths::default_log_dir;
pub use rolling::RollingFile;

/// 单个日志文件的默认字节上限（8 MiB）。
pub const DEFAULT_MAX_BYTES: u64 = 8 * 1024 * 1024;
/// 归档份数默认上限；盘上文件总量 = 当前 + 归档数（见 rolling 模块文档）。
pub const DEFAULT_MAX_FILES: usize = 5;

/// 输出格式：文本（人类可读）或 JSON（机器可解析）。
///
/// 只作用于文件层；stderr 层恒为文本，保证控制台可读。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LogFormat {
    #[default]
    Text,
    Json,
}

/// 滚动文件落盘选项。
#[derive(Debug, Clone)]
pub struct FileOptions {
    /// 日志目录（不存在时会创建）。
    pub dir: PathBuf,
    /// 当前日志文件名（如 "node.log"，归档为 "node.log.1" 递增）。
    pub name: String,
    /// 单文件字节上限，超出后滚动。
    pub max_bytes: u64,
    /// 归档份数上限，超出后丢弃最老归档。
    pub max_files: usize,
}

impl FileOptions {
    /// 用设施默认上限（[DEFAULT_MAX_BYTES] / [DEFAULT_MAX_FILES]）构造。
    pub fn with_default_caps(dir: impl Into<PathBuf>, name: impl Into<String>) -> Self {
        Self {
            dir: dir.into(),
            name: name.into(),
            max_bytes: DEFAULT_MAX_BYTES,
            max_files: DEFAULT_MAX_FILES,
        }
    }
}

/// 初始化配置：输出格式 + 可选滚动文件。
#[derive(Debug, Clone, Default)]
pub struct LogConfig {
    pub format: LogFormat,
    pub file: Option<FileOptions>,
}

/// 初始化结果报告。
#[derive(Debug, Clone)]
pub struct InitReport {
    /// 当前生效的日志文件路径；None = 仅 stderr。
    pub file_path: Option<PathBuf>,
    /// 非空 = 初始化存在失败并被回退处理（告警文本，已同时 eprintln）。
    pub fallback: Option<String>,
    /// true = 本次调用是重复初始化，直接返回首次初始化的结果，配置未生效。
    pub already_initialized: bool,
}

static REPORT: OnceLock<InitReport> = OnceLock::new();

/// 初始化全局日志设施；重复调用幂等（返回首次结果并置 already_initialized）。
///
/// 成功时同时安装 panic 钩子；订阅器安装失败（全局位被占）同样留告警。
pub fn init(config: LogConfig) -> InitReport {
    if let Some(first) = REPORT.get() {
        return InitReport {
            already_initialized: true,
            ..first.clone()
        };
    }
    let report = init_once(config);
    let _ = REPORT.set(report.clone());
    report
}

fn init_once(config: LogConfig) -> InitReport {
    // panic 钩子先行安装：无论订阅器归属如何，panic 都可观测。
    panic::install();
    let mut report = InitReport {
        file_path: None,
        fallback: None,
        already_initialized: false,
    };
    let filter = build_filter(std::env::var("RUST_LOG").ok().as_deref());
    let attempt = match &config.file {
        None => FileAttempt::Off,
        Some(opts) => {
            match RollingFile::new(opts.dir.join(&opts.name), opts.max_bytes, opts.max_files) {
                Ok(rolling) => FileAttempt::Ready(rolling, opts.dir.join(&opts.name)),
                Err(e) => FileAttempt::Failed(format!(
                    "日志落盘初始化失败（dir={}），回退 stderr 输出: {e}",
                    opts.dir.display()
                )),
            }
        }
    };
    let mounted = match attempt {
        FileAttempt::Ready(rolling, path) => {
            report.file_path = Some(path);
            mount(filter, Some((rolling, config.format)))
        }
        FileAttempt::Off => mount(filter, None),
        FileAttempt::Failed(msg) => {
            eprintln!("p2p-log: {msg}");
            report.fallback = Some(msg);
            mount(filter, None)
        }
    };
    if let Err(e) = mounted {
        let msg = format!("日志订阅器安装失败（全局订阅器已被占用?），本配置未生效: {e}");
        eprintln!("p2p-log: {msg}");
        report.fallback = Some(match report.fallback {
            Some(prev) => format!("{prev}; {msg}"),
            None => msg,
        });
    }
    report
}

/// 装配订阅器：stderr 文本层 + 可选文件层（共享同一 EnvFilter）。
fn mount(
    filter: EnvFilter,
    file: Option<(RollingFile, LogFormat)>,
) -> Result<(), tracing_subscriber::util::TryInitError> {
    match file {
        Some((rolling, LogFormat::Json)) => tracing_subscriber::registry()
            .with(filter)
            .with(fmt::layer().with_writer(std::io::stderr))
            .with(fmt::layer().with_ansi(false).json().with_writer(rolling))
            .try_init(),
        Some((rolling, LogFormat::Text)) => tracing_subscriber::registry()
            .with(filter)
            .with(fmt::layer().with_writer(std::io::stderr))
            .with(fmt::layer().with_ansi(false).with_writer(rolling))
            .try_init(),
        None => tracing_subscriber::registry()
            .with(filter)
            .with(fmt::layer().with_writer(std::io::stderr))
            .try_init(),
    }
}

/// 由 RUST_LOG 环境变量值构建过滤器；未设置或为空时默认 info。
///
/// 非法指令按 tracing-subscriber 的 lossy 语义忽略（保留默认 info）。
pub fn build_filter(rust_log: Option<&str>) -> EnvFilter {
    let value = rust_log.unwrap_or("");
    EnvFilter::builder()
        .with_default_directive(LevelFilter::INFO.into())
        .parse_lossy(value)
}

enum FileAttempt {
    Ready(RollingFile, PathBuf),
    Failed(String),
    Off,
}

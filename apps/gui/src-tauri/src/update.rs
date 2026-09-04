//! 在线更新检查（契约 v4 加法 §9，G-U1）：查 GitHub 最新稳定 release 并与当前版本比较。
//!
//! 无状态：不缓存不轮询，节奏由前端驱动；网络经 ReleasesSource 注入，
//! 测试喂固定响应，严禁真实外网。候选过滤与失败语义见 github.rs / 契约 §9。

pub mod github;
#[cfg(test)]
mod tests;
pub mod version;

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use tauri::{AppHandle, Runtime, State};
use tracing::{debug, error, warn};

use crate::update::github::{GitHubSource, LatestRelease};

/// 数据源调用的返回 future（手工装箱保持 trait 对象安全）。
pub type FetchFuture = Pin<Box<dyn Future<Output = Result<String, String>> + Send>>;

/// Releases 数据源抽象：生产走真实 HTTP，测试喂固定响应。
pub trait ReleasesSource: Send + Sync {
    /// 返回 GitHub releases API 的原始 JSON 文本；失败给可读中文。
    fn fetch_releases_json(&self) -> FetchFuture;
}

/// 契约 §9 UpdateCheckResult：字段名 camelCase，Option 序列化为 null。
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateCheckResult {
    /// 应用当前版本（tauri.conf version）。
    pub current_version: String,
    /// 无候选时 null。
    pub latest_version: Option<String>,
    pub has_update: bool,
    /// release html_url。
    pub release_url: Option<String>,
    pub release_name: Option<String>,
    /// release body 原文。
    pub release_notes_md: Option<String>,
    pub published_at_ms: Option<i64>,
    pub checked_at_ms: i64,
}

/// 更新检查状态：持有注入的数据源（生产 GitHubSource）。
pub struct UpdateChecker {
    source: Arc<dyn ReleasesSource>,
}

impl UpdateChecker {
    pub fn new() -> Result<Self, String> {
        Ok(Self {
            source: Arc::new(GitHubSource::new()?),
        })
    }
}

/// 检查核心：解析当前版本 → 拉取 → 选最新稳定候选 → 逐段数值比较（契约 §9）。
pub async fn run_check(
    source: &dyn ReleasesSource,
    current_version: &str,
    checked_at_ms: i64,
) -> Result<UpdateCheckResult, String> {
    let current = version::parse_tag(current_version)
        .map_err(|e| format!("当前应用版本解析失败: {e}"))
        .inspect_err(|e| error!("{e}"))?;
    let body = source.fetch_releases_json().await?;
    let base = UpdateCheckResult {
        current_version: current_version.to_string(),
        latest_version: None,
        has_update: false,
        release_url: None,
        release_name: None,
        release_notes_md: None,
        published_at_ms: None,
        checked_at_ms,
    };
    let Some(latest) = github::latest_stable(&body)? else {
        debug!("无满足条件的稳定 release 候选");
        return Ok(base);
    };
    let has_update = version::compare(&latest.version, &current) == std::cmp::Ordering::Greater;
    let published_at_ms = published_at_ms(&latest);
    Ok(UpdateCheckResult {
        latest_version: Some(latest.tag),
        has_update,
        release_url: Some(latest.html_url),
        release_name: latest.name,
        release_notes_md: latest.body,
        published_at_ms,
        ..base
    })
}

/// published_at（RFC3339）→ 毫秒时间戳；非法值降级为 null 并告警，不阻断检查。
fn published_at_ms(release: &LatestRelease) -> Option<i64> {
    let raw = release.published_at.as_deref()?;
    match chrono::DateTime::parse_from_rfc3339(raw) {
        Ok(t) => Some(t.timestamp_millis()),
        Err(e) => {
            warn!(
                "release {} 的 published_at 非法，按缺失处理: {raw} ({e})",
                release.tag
            );
            None
        }
    }
}

/// update_check：查询最新稳定 release 并与当前版本比较（契约 §9）。
#[tauri::command]
pub async fn update_check<R: Runtime>(
    app: AppHandle<R>,
    checker: State<'_, UpdateChecker>,
) -> Result<UpdateCheckResult, String> {
    let current = app.package_info().version.to_string();
    run_check(checker.source.as_ref(), &current, now_ms()).await
}

/// update_open_release_page：白名单校验通过后交系统默认程序打开（契约 §9）。
#[tauri::command]
pub async fn update_open_release_page<R: Runtime>(
    app: AppHandle<R>,
    url: String,
) -> Result<(), String> {
    if let Err(e) = validate_release_url(&url) {
        warn!("打开发布页被拒（白名单外）: {url}");
        return Err(e);
    }
    use tauri_plugin_opener::OpenerExt;
    app.opener().open_url(url, None::<&str>).map_err(|e| {
        error!("系统打开发布页失败: {e}");
        format!("打开发布页失败: {e}")
    })
}

/// 白名单：https 协议且 host 恰为 github.com（契约 §9），防任意 URL 外跳。
pub fn validate_release_url(url: &str) -> Result<(), String> {
    let parsed = url::Url::parse(url).map_err(|_| format!("URL 无法解析: {url}"))?;
    if parsed.scheme() != "https" {
        return Err(format!("只允许 https 链接: {url}"));
    }
    if parsed.host_str() != Some("github.com") {
        return Err(format!("只允许 github.com 链接: {url}"));
    }
    Ok(())
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or_else(|e| {
            error!("系统时钟早于 Unix 纪元: {e}");
            0
        })
}

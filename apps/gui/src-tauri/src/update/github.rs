//! GitHub Releases API（契约 v4 §9）：响应模型、候选过滤与生产 HTTP 数据源。

use std::cmp::Ordering;
use std::time::Duration;

use serde::Deserialize;
use tracing::{debug, error};

use crate::update::version::{self, SemVer};

/// GitHub Releases 公开只读端点（契约 §9：编译期常量，不做用户配置）。
pub const RELEASES_ENDPOINT: &str =
    "https://api.github.com/repos/imeepos/p2p/releases?per_page=10";

/// HTTP 超时（契约 §9：10s）。
const HTTP_TIMEOUT: Duration = Duration::from_secs(10);

/// GitHub 拒绝无 User-Agent 请求；UA 带应用版本便于排障。
const USER_AGENT: &str = concat!("p2p-console/", env!("CARGO_PKG_VERSION"));

const ACCEPT_JSON: &str = "application/vnd.github+json";

/// GitHub release 条目中本功能关心的字段子集（多余字段忽略）。
#[derive(Debug, Deserialize)]
struct ReleaseEntry {
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    prerelease: bool,
    tag_name: String,
    html_url: String,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    body: Option<String>,
    #[serde(default)]
    published_at: Option<String>,
}

/// 最新稳定候选：tag 已解析为版本。
pub struct LatestRelease {
    pub version: SemVer,
    pub tag: String,
    pub html_url: String,
    pub name: Option<String>,
    pub body: Option<String>,
    pub published_at: Option<String>,
}

/// 响应文本选最新一条稳定候选：仅取 draft=false 且 prerelease=false 且 tag 可解析
/// 为三段版本者，按版本取最大；无候选返回 None。响应不是 release 数组视为非法（Err）。
pub fn latest_stable(body: &str) -> Result<Option<LatestRelease>, String> {
    let entries: Vec<ReleaseEntry> = serde_json::from_str(body)
        .map_err(|e| format!("GitHub Releases 响应非法: {e}"))?;
    let mut best: Option<(SemVer, LatestRelease)> = None;
    for entry in entries {
        if entry.draft || entry.prerelease {
            continue;
        }
        let ver = match version::parse_tag(&entry.tag_name) {
            Ok(v) => v,
            Err(e) => {
                debug!("跳过非三段版本 tag {}: {e}", entry.tag_name);
                continue;
            }
        };
        let is_newer = best
            .as_ref()
            .is_none_or(|(v, _)| version::compare(&ver, v) == Ordering::Greater);
        if is_newer {
            best = Some((
                ver.clone(),
                LatestRelease {
                    version: ver,
                    tag: entry.tag_name,
                    html_url: entry.html_url,
                    name: entry.name,
                    body: entry.body,
                    published_at: entry.published_at,
                },
            ));
        }
    }
    Ok(best.map(|(_, release)| release))
}

/// 生产数据源：真实 HTTP（10s 超时 + 自定义 UA，契约 §9）。
pub struct GitHubSource {
    client: reqwest::Client,
}

impl GitHubSource {
    pub fn new() -> Result<Self, String> {
        let client = reqwest::Client::builder()
            .timeout(HTTP_TIMEOUT)
            .user_agent(USER_AGENT)
            .build()
            .map_err(|e| format!("构造更新检查 HTTP 客户端失败: {e}"))?;
        Ok(Self { client })
    }
}

impl crate::update::ReleasesSource for GitHubSource {
    fn fetch_releases_json(
        &self,
    ) -> crate::update::FetchFuture {
        let client = self.client.clone();
        Box::pin(async move {
            let resp = client
                .get(RELEASES_ENDPOINT)
                .header(reqwest::header::ACCEPT, ACCEPT_JSON)
                .send()
                .await
                .map_err(|e| {
                    error!("请求 GitHub Releases 失败: {e}");
                    format!("网络请求 GitHub Releases 失败: {e}")
                })?;
            let status = resp.status();
            let body = resp.text().await.map_err(|e| {
                error!("读取 GitHub Releases 响应失败: {e}");
                format!("读取 GitHub Releases 响应失败: {e}")
            })?;
            if !status.is_success() {
                error!("GitHub Releases 响应状态异常: {status}");
                return Err(format!("GitHub Releases 响应异常（HTTP {status}）"));
            }
            Ok(body)
        })
    }
}
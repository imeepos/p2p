//! Releases 数据源抽象与生产 HTTP 实现：网络经 trait 注入，测试喂固定响应，
//! 严禁真实外网（同 GUI update.rs 约定）。HTTP 用 reqwest + rustls（同 GUI 选型）。

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use crate::update::github::RELEASES_ENDPOINT;

/// HTTP 超时（契约 §9：10s）。
const HTTP_TIMEOUT: Duration = Duration::from_secs(10);

/// GitHub 拒绝无 User-Agent 请求；UA 带应用名便于排障。
const USER_AGENT: &str = concat!("p2pctl/", env!("CARGO_PKG_VERSION"));
const ACCEPT_JSON: &str = "application/vnd.github+json";

/// 数据源调用的返回 future（手工装箱保持 trait 对象安全）。
pub type FetchFuture = Pin<Box<dyn Future<Output = Result<String, String>> + Send>>;

/// Releases 数据源抽象：生产走真实 HTTP，测试喂固定响应。
pub trait ReleasesSource: Send + Sync {
    /// 返回 GitHub releases API 的原始 JSON 文本；失败给可读中文。
    fn fetch_releases_json(&self) -> FetchFuture;
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

impl ReleasesSource for GitHubSource {
    fn fetch_releases_json(&self) -> FetchFuture {
        let client = self.client.clone();
        Box::pin(async move {
            let resp = client
                .get(RELEASES_ENDPOINT)
                .header(reqwest::header::ACCEPT, ACCEPT_JSON)
                .send()
                .await
                .map_err(|e| format!("网络请求 GitHub Releases 失败: {e}"))?;
            let status = resp.status();
            let body = resp
                .text()
                .await
                .map_err(|e| format!("读取 GitHub Releases 响应失败: {e}"))?;
            if !status.is_success() {
                eprintln!("p2pctl: GitHub Releases 响应状态异常: {status}");
                return Err(format!("GitHub Releases 响应异常（HTTP {status}）"));
            }
            Ok(body)
        })
    }
}

/// 共享句柄形态：CLI 单次调用直接持实例即可。
pub fn github_source() -> Result<Arc<dyn ReleasesSource>, String> {
    Ok(Arc::new(GitHubSource::new()?))
}

#[cfg(test)]
mod tests {
    use super::*;

    struct FixedSource(&'static str);

    impl ReleasesSource for FixedSource {
        fn fetch_releases_json(&self) -> FetchFuture {
            let body = self.0.to_string();
            Box::pin(async move { Ok(body) })
        }
    }

    #[tokio::test]
    async fn fixed_source_feeds_body_without_network() {
        let src = FixedSource(r#"[{"tag_name":"v0.2.0","html_url":"https://github.com/imeepos/p2p/releases/tag/v0.2.0"}]"#);
        assert!(src.fetch_releases_json().await.unwrap().contains("v0.2.0"));
    }
}

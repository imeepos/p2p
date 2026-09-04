//! update 命令域（CL4）：对齐 GUI update_check / update_open_release_page（契约 §9）。
//! check 拉取 GitHub 最新稳定 release 并与当前版本比较；open 在 CLI 语义为
//! 校验白名单后输出 release 页 URL（不开浏览器），映射说明见 cli-parity.tsv。

mod github;
mod source;
mod version;

use clap::Subcommand;
use serde::Serialize;

use crate::error::{CliError, CliResult};
use crate::output;
use source::{github_source, ReleasesSource};

#[derive(Subcommand)]
pub enum UpdateCommand {
    /// 检查 GitHub 最新稳定 release 并与当前版本比较
    Check(CheckArgs),
    /// 输出 release 页 URL（CLI 语义：不开浏览器；缺省 --url 时先检查取最新候选）
    Open(OpenArgs),
}

#[derive(clap::Args)]
pub struct CheckArgs {
    /// 输出结构化 JSON
    #[arg(long)]
    json: bool,
}

#[derive(clap::Args)]
pub struct OpenArgs {
    /// 直接校验并输出该 URL（https + github.com 白名单）；缺省则先检查取最新候选
    #[arg(long)]
    url: Option<String>,
    /// 输出结构化 JSON
    #[arg(long)]
    json: bool,
}

/// 检查结论：字段名对齐 GUI 契约 §9 UpdateCheckResult（camelCase）。
#[derive(Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
struct CheckReport {
    current_version: String,
    latest_version: Option<String>,
    has_update: bool,
    release_url: Option<String>,
    release_name: Option<String>,
    release_notes_md: Option<String>,
    published_at: Option<String>,
    checked_at_ms: i64,
}

pub async fn run(cmd: UpdateCommand) -> CliResult<()> {
    match cmd {
        UpdateCommand::Check(a) => check(a).await,
        UpdateCommand::Open(a) => open(a).await,
    }
}

async fn check(args: CheckArgs) -> CliResult<()> {
    let source = github_source().map_err(CliError::Runtime)?;
    let report = run_check(
        source.as_ref(),
        env!("CARGO_PKG_VERSION"),
        now_ms(),
    )
    .await
    .map_err(CliError::Runtime)?;
    let text = render_check(&report);
    output::emit(args.json, &report, &text)
}

async fn open(args: OpenArgs) -> CliResult<()> {
    let url = match &args.url {
        Some(url) => {
            validate_release_url(url).map_err(CliError::Runtime)?;
            url.clone()
        }
        None => {
            let source = github_source().map_err(CliError::Runtime)?;
            let report = run_check(source.as_ref(), env!("CARGO_PKG_VERSION"), now_ms())
                .await
                .map_err(CliError::Runtime)?;
            report
                .release_url
                .ok_or_else(|| CliError::Runtime("无稳定 release 候选，没有可输出的发布页".into()))?
        }
    };
    let report = OpenReport { url: url.clone() };
    output::emit(args.json, &report, &url)
}

#[derive(Serialize)]
struct OpenReport {
    url: String,
}

/// 检查核心（语义同 GUI run_check）：解析当前版本 → 拉取 → 选最新稳定候选 → 比较。
async fn run_check(
    src: &dyn ReleasesSource,
    current_version: &str,
    checked_at_ms: i64,
) -> Result<CheckReport, String> {
    let current = version::parse_tag(current_version)?;
    let body = src.fetch_releases_json().await?;
    let Some(latest) = github::latest_stable(&body)? else {
        return Ok(CheckReport {
            current_version: current_version.to_string(),
            latest_version: None,
            has_update: false,
            release_url: None,
            release_name: None,
            release_notes_md: None,
            published_at: None,
            checked_at_ms,
        });
    };
    let has_update = version::compare(&latest.version, &current) == std::cmp::Ordering::Greater;
    Ok(CheckReport {
        latest_version: Some(latest.tag),
        has_update,
        release_url: Some(latest.html_url),
        release_name: latest.name,
        release_notes_md: latest.body,
        published_at: latest.published_at,
        current_version: current_version.to_string(),
        checked_at_ms,
    })
}

/// 白名单：https 协议且 host 恰为 github.com（契约 §9），防任意 URL 外跳。
fn validate_release_url(url: &str) -> Result<(), String> {
    let parsed = url::Url::parse(url).map_err(|_| format!("URL 无法解析: {url}"))?;
    if parsed.scheme() != "https" {
        return Err(format!("只允许 https 链接: {url}"));
    }
    if parsed.host_str() != Some("github.com") {
        return Err(format!("只允许 github.com 链接: {url}"));
    }
    Ok(())
}

fn render_check(r: &CheckReport) -> String {
    let mut lines = vec![format!("current={}", r.current_version)];
    match (&r.latest_version, &r.release_url) {
        (Some(tag), url) => {
            lines.push(format!("latest={tag}"));
            lines.push(format!("hasUpdate={}", r.has_update));
            if let Some(url) = url {
                lines.push(format!("url={url}"));
            }
            if let Some(name) = &r.release_name {
                lines.push(format!("name={name}"));
            }
        }
        (None, _) => lines.push("latest=（无稳定 release 候选）".into()),
    }
    lines.join("\n")
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::update::source::FetchFuture;

    struct FixedSource(&'static str);

    impl ReleasesSource for FixedSource {
        fn fetch_releases_json(&self) -> FetchFuture {
            let body = self.0.to_string();
            Box::pin(async move { Ok(body) })
        }
    }

    const BODY: &str = r#"[{"tag_name":"client-v0.9.0","html_url":"https://github.com/imeepos/p2p/releases/tag/client-v0.9.0","name":"v0.9.0","body":"n"}]"#;

    #[tokio::test]
    async fn check_detects_update_against_older_current() {
        let report = run_check(&FixedSource(BODY), "0.1.0", 42).await.unwrap();
        assert!(report.has_update);
        assert_eq!(report.latest_version.as_deref(), Some("client-v0.9.0"));
        assert_eq!(report.checked_at_ms, 42);
    }

    #[tokio::test]
    async fn check_no_candidate_reports_none() {
        let report = run_check(&FixedSource("[]"), "0.1.0", 1).await.unwrap();
        assert!(!report.has_update);
        assert!(report.latest_version.is_none());
        assert!(render_check(&report).contains("无稳定 release 候选"));
    }

    #[test]
    fn whitelist_blocks_non_github_urls() {
        assert!(validate_release_url("https://github.com/imeepos/p2p/releases").is_ok());
        assert!(validate_release_url("http://github.com/x").is_err());
        assert!(validate_release_url("https://evil.com/x").is_err());
        assert!(validate_release_url("not a url").is_err());
    }
}

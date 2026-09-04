//! GitHub Releases 响应选优：仅取稳定候选（draft/prerelease 排除、tag 可解析），
//! 按三段版本取最大；语义对齐 apps/gui update/github.rs（契约 §9）。

use std::cmp::Ordering;

use serde::Deserialize;

use crate::update::version::{self, SemVer};

/// GitHub Releases 公开只读端点（契约 §9：编译期常量，不做用户配置）。
pub const RELEASES_ENDPOINT: &str =
    "https://api.github.com/repos/imeepos/p2p/releases?per_page=10";

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

/// 响应文本选最新一条稳定候选；响应不是 release 数组视为非法（Err）。
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
            Err(_) => continue, // 非三段版本 tag 不是候选，跳过
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

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(tag: &str, draft: bool, pre: bool) -> String {
        format!(r#"{{"tag_name":"{tag}","html_url":"https://github.com/imeepos/p2p/releases/tag/{tag}","draft":{draft},"prerelease":{pre},"name":"n-{tag}","body":"notes","published_at":"2026-01-01T00:00:00Z"}}"#)
    }

    #[test]
    fn picks_newest_stable_candidate() {
        let body = format!("[{},{},{}]", entry("client-v0.1.1", false, false), entry("client-v0.1.10", false, false), entry("client-v0.2.0", true, false));
        let best = latest_stable(&body).unwrap().unwrap();
        assert_eq!(best.tag, "client-v0.1.10");
        assert_eq!(best.version, SemVer { major: 0, minor: 1, patch: 10 });
        assert_eq!(best.name.as_deref(), Some("n-client-v0.1.10"));
    }

    #[test]
    fn no_candidate_returns_none_and_bad_body_is_err() {
        let only_pre = format!("[{}]", entry("v9.9.9", false, true));
        assert!(latest_stable(&only_pre).unwrap().is_none());
        assert!(latest_stable("not-an-array").is_err());
        assert!(latest_stable("[]").unwrap().is_none());
    }
}

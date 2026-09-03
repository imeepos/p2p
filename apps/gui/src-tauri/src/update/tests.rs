//! 契约 v4 §9 用例：候选过滤、tag 三形态、逐段数值比较、失败语义、
//! serde roundtrip、白名单正反。全部走注入源固定响应，零真实外网。

use std::cmp::Ordering;

use super::*;

/// 固定响应源：Ok(json) 或 Err(网络失败)。
struct FixedSource(Result<String, String>);

impl ReleasesSource for FixedSource {
    fn fetch_releases_json(&self) -> FetchFuture {
        let body = self.0.clone();
        Box::pin(async move { body })
    }
}

/// 单条 release 条目的最小合法 JSON。
fn release_json(tag: &str, draft: bool, prerelease: bool) -> String {
    format!(
        r#"{{"tag_name":"{tag}","draft":{draft},"prerelease":{prerelease},
"html_url":"https://github.com/imeepos/p2p/releases/tag/{tag}",
"name":"p2p-console {tag}","body":"- notes for {tag}",
"published_at":"2026-09-01T08:15:30Z"}}"#
    )
}

fn releases_array(items: &[String]) -> String {
    format!("[{}]", items.join(","))
}

async fn check_current(current: &str, body: Result<String, String>) -> Result<UpdateCheckResult, String> {
    run_check(&FixedSource(body), current, 1_700_000_000_000).await
}

#[tokio::test]
async fn update_available_maps_candidate_fields() {
    let body = releases_array(&[
        release_json("client-v0.1.0", false, false),
        release_json("client-v0.2.0", false, false),
    ]);
    let r = check_current("0.1.0", Ok(body)).await.unwrap();
    assert!(r.has_update);
    assert_eq!(r.latest_version.as_deref(), Some("client-v0.2.0"));
    assert_eq!(r.current_version, "0.1.0");
    assert_eq!(
        r.release_url.as_deref(),
        Some("https://github.com/imeepos/p2p/releases/tag/client-v0.2.0")
    );
    assert_eq!(r.release_name.as_deref(), Some("p2p-console client-v0.2.0"));
    assert_eq!(r.release_notes_md.as_deref(), Some("- notes for client-v0.2.0"));
    assert_eq!(r.published_at_ms, Some(1_788_250_530_000));
    assert_eq!(r.checked_at_ms, 1_700_000_000_000);
}

#[tokio::test]
async fn no_candidates_yields_null_latest() {
    let body = releases_array(&[
        release_json("client-v0.3.0", true, false),
        release_json("client-v0.4.0", false, true),
    ]);
    let r = check_current("0.1.0", Ok(body)).await.unwrap();
    assert_eq!(r.latest_version, None);
    assert!(!r.has_update);
    assert_eq!(r.release_url, None);
    assert_eq!(r.published_at_ms, None);
}

#[tokio::test]
async fn empty_releases_array_is_not_error() {
    let r = check_current("0.1.0", Ok("[]".into())).await.unwrap();
    assert_eq!(r.latest_version, None);
    assert!(!r.has_update);
}

#[tokio::test]
async fn tag_prefix_forms_all_recognized() {
    for tag in ["client-v0.2.0", "v0.2.0", "0.2.0"] {
        let body = releases_array(&[release_json(tag, false, false)]);
        let r = check_current("0.1.0", Ok(body)).await.unwrap();
        assert!(r.has_update, "tag {tag} 应识别为候选");
        assert_eq!(r.latest_version.as_deref(), Some(tag));
    }
}

#[tokio::test]
async fn prerelease_only_returns_null() {
    let body = releases_array(&[release_json("client-v9.9.9", false, true)]);
    let r = check_current("0.1.0", Ok(body)).await.unwrap();
    assert_eq!(r.latest_version, None);
}

#[tokio::test]
async fn latest_wins_regardless_of_entry_order() {
    let body = releases_array(&[
        release_json("client-v0.3.0", false, false),
        release_json("client-v0.2.0", false, false),
    ]);
    let r = check_current("0.1.0", Ok(body)).await.unwrap();
    assert_eq!(r.latest_version.as_deref(), Some("client-v0.3.0"));
}

#[tokio::test]
async fn picks_0_10_0_over_0_9_0_by_number() {
    let body = releases_array(&[
        release_json("v0.9.0", false, false),
        release_json("v0.10.0", false, false),
    ]);
    let r = check_current("0.9.0", Ok(body)).await.unwrap();
    assert!(r.has_update, "0.10.0 应大于 0.9.0（逐段数值而非字符串）");
    assert_eq!(r.latest_version.as_deref(), Some("v0.10.0"));
}

#[test]
fn numeric_segment_compare_pins_0_10_over_0_9() {
    let v9 = version::parse_tag("0.9.0").unwrap();
    let v10 = version::parse_tag("0.10.0").unwrap();
    assert_eq!(version::compare(&v10, &v9), Ordering::Greater);
    assert_eq!(version::compare(&v9, &v10), Ordering::Less);
    assert_eq!(version::compare(&v10, &v10), Ordering::Equal);
    let v1 = version::parse_tag("1.0.0").unwrap();
    assert_eq!(version::compare(&v1, &v10), Ordering::Greater);
}

#[tokio::test]
async fn network_failure_is_readable_err() {
    let err = check_current("0.1.0", Err("网络请求 GitHub Releases 失败: 连接超时".into()))
        .await
        .unwrap_err();
    assert!(err.contains("网络"), "失败信息应可读中文: {err}");
}

#[tokio::test]
async fn invalid_json_is_err() {
    let err = check_current("0.1.0", Ok("<html>Gateway Timeout</html>".into()))
        .await
        .unwrap_err();
    assert!(err.contains("非法"), "失败信息应可读中文: {err}");
}

#[tokio::test]
async fn non_array_json_is_err() {
    let err = check_current("0.1.0", Ok("\"just a string\"".into())).await.unwrap_err();
    assert!(err.contains("非法"));
}

#[tokio::test]
async fn broken_current_version_is_err() {
    let body = releases_array(&[release_json("v0.2.0", false, false)]);
    let err = check_current("dev-build", Ok(body)).await.unwrap_err();
    assert!(err.contains("当前应用版本解析失败"));
}

#[tokio::test]
async fn bad_published_at_degrades_to_null() {
    let entry = r#"{"tag_name":"v0.2.0","draft":false,"prerelease":false,
"html_url":"https://github.com/imeepos/p2p/releases/tag/v0.2.0",
"published_at":"not-a-time"}"#
        .to_string();
    let r = check_current("0.1.0", Ok(releases_array(&[entry]))).await.unwrap();
    assert!(r.has_update);
    assert_eq!(r.published_at_ms, None);
}

#[test]
fn update_check_result_serde_roundtrip_and_null() {
    let full = UpdateCheckResult {
        current_version: "0.1.0".into(),
        latest_version: Some("client-v0.2.0".into()),
        has_update: true,
        release_url: Some("https://github.com/imeepos/p2p/releases/tag/client-v0.2.0".into()),
        release_name: Some("p2p-console client-v0.2.0".into()),
        release_notes_md: Some("- notes".into()),
        published_at_ms: Some(1_788_250_530_000),
        checked_at_ms: 1_700_000_000_000,
    };
    let json = serde_json::to_string(&full).unwrap();
    for key in [
        "currentVersion",
        "latestVersion",
        "hasUpdate",
        "releaseUrl",
        "releaseName",
        "releaseNotesMd",
        "publishedAtMs",
        "checkedAtMs",
    ] {
        assert!(json.contains(&format!("\"{key}\"")), "缺字段 {key}: {json}");
    }
    assert_eq!(serde_json::from_str::<UpdateCheckResult>(&json).unwrap(), full);

    let none = UpdateCheckResult {
        latest_version: None,
        release_url: None,
        release_name: None,
        release_notes_md: None,
        published_at_ms: None,
        ..full.clone()
    };
    let json_none = serde_json::to_string(&none).unwrap();
    assert!(json_none.contains("\"latestVersion\":null"), "Option 应序列化为 null: {json_none}");
    assert_eq!(serde_json::from_str::<UpdateCheckResult>(&json_none).unwrap(), none);
}

#[test]
fn release_url_whitelist() {
    assert!(validate_release_url("https://github.com/imeepos/p2p/releases/tag/client-v0.2.0").is_ok());
    assert!(validate_release_url("https://GitHub.com/imeepos/p2p").is_ok());
    assert!(validate_release_url("http://github.com/imeepos/p2p").is_err());
    assert!(validate_release_url("https://api.github.com/repos/imeepos/p2p").is_err());
    assert!(validate_release_url("https://github.com.evil.com/releases").is_err());
    assert!(validate_release_url("https://evil.com/github.com").is_err());
    assert!(validate_release_url("not a url").is_err());
}
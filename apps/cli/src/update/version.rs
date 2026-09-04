//! 三段语义版本解析与逐段数值比较（契约 v4 §9：禁止字符串比较）。
//! 与 apps/gui/src-tauri/src/update/version.rs 同语义：GUI crate 不可依赖
//! （Tauri 应用不进底座 workspace），对等面按契约各自实现并各自测试。

use std::cmp::Ordering;

/// major.minor.patch 三段版本。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SemVer {
    pub major: u64,
    pub minor: u64,
    pub patch: u64,
}

/// 解析 release tag / 应用版本：容忍 client-v、v 前缀与裸三段三种形态。
pub fn parse_tag(tag: &str) -> Result<SemVer, String> {
    let body = tag
        .strip_prefix("client-v")
        .or_else(|| tag.strip_prefix('v'))
        .unwrap_or(tag);
    let parts: Vec<&str> = body.split('.').collect();
    if parts.len() != 3 {
        return Err(format!("版本必须是三段数字: {tag}"));
    }
    let mut segs = [0u64; 3];
    for (slot, part) in parts.iter().enumerate() {
        if part.is_empty() || !part.bytes().all(|b| b.is_ascii_digit()) {
            return Err(format!("版本段必须是非空纯数字: {tag}"));
        }
        segs[slot] = part
            .parse()
            .map_err(|_| format!("版本段数值溢出: {tag}"))?;
    }
    Ok(SemVer { major: segs[0], minor: segs[1], patch: segs[2] })
}

/// 逐段数值比较（0.10.0 > 0.9.0）。
pub fn compare(a: &SemVer, b: &SemVer) -> Ordering {
    a.major
        .cmp(&b.major)
        .then(a.minor.cmp(&b.minor))
        .then(a.patch.cmp(&b.patch))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_tag_prefixes_and_plain() {
        for tag in ["client-v0.1.2", "v0.1.2", "0.1.2"] {
            assert_eq!(parse_tag(tag).unwrap(), SemVer { major: 0, minor: 1, patch: 2 }, "{tag}");
        }
    }

    #[test]
    fn rejects_malformed_tags() {
        for tag in ["1.2", "a.b.c", "1.2.x", "1..3", ""] {
            assert!(parse_tag(tag).is_err(), "{tag} 应被拒绝");
        }
    }

    #[test]
    fn compares_numerically_not_lexically() {
        let a = parse_tag("0.10.0").unwrap();
        let b = parse_tag("0.9.0").unwrap();
        assert_eq!(compare(&a, &b), Ordering::Greater);
        assert_eq!(compare(&b, &a), Ordering::Less);
        assert_eq!(compare(&a, &a), Ordering::Equal);
    }
}

//! 三段语义版本解析与逐段数值比较（契约 v4 §9：禁止字符串比较）。

use std::cmp::Ordering;

/// major.minor.patch 三段版本。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SemVer {
    pub major: u64,
    pub minor: u64,
    pub patch: u64,
}

/// 解析 release tag / 应用版本：容忍 client-v、v 前缀与裸三段三种形态。
/// 非三段、段含非数字字符返回 Err；调用方按语义决定过滤候选或报错。
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
    Ok(SemVer {
        major: segs[0],
        minor: segs[1],
        patch: segs[2],
    })
}

/// 逐段数值比较（0.10.0 > 0.9.0）。
pub fn compare(a: &SemVer, b: &SemVer) -> Ordering {
    a.major
        .cmp(&b.major)
        .then(a.minor.cmp(&b.minor))
        .then(a.patch.cmp(&b.patch))
}

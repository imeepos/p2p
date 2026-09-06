//! dsh-acp-share://v1 链接解析（docs/design/acp-share-design.md §2 冻结契约）：
//! scheme/版本不符即拒绝；缺 peer/token 即拒绝；addr 可重复（多地址候选）；
//! exp/sid 仅展示提示，guest 不做本地时钟判定；未知参数忽略。
//! token 原样透传不归一化；错误信息不回显 token 原文。

use p2p::PeerId;

/// 链接 scheme（比较不区分大小写）。
pub const SCHEME: &str = "dsh-acp-share";
/// 链接版本（authority 段）。
pub const VERSION: &str = "v1";
/// token 宽度：128-bit 随机 hex（§2）。
pub const TOKEN_HEX_LEN: usize = 32;

/// 解析产物：peer 为拨号目标，addrs 为链接序的地址候选，token 透传给握手。
#[derive(Clone, Debug)]
pub struct ShareLink {
    pub peer: String,
    pub peer_id: PeerId,
    pub addrs: Vec<String>,
    pub token: String,
    /// 过期时刻（unix 秒）展示提示；权威判定在 agent 侧（§2），不参与本端拒绝。
    pub exp: Option<String>,
    /// shareId 展示提示。
    pub sid: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum ShareLinkError {
    #[error("not a dsh-acp-share link (want dsh-acp-share://v1?...)")]
    NotShareLink,
    #[error("unsupported share link version: {0}")]
    BadVersion(String),
    #[error("share link missing peer")]
    MissingPeer,
    #[error("share link bad peer: {0}")]
    BadPeer(String),
    #[error("share link missing token")]
    MissingToken,
    #[error("share link token must be 32 hex chars")]
    BadToken,
    #[error("share link has empty addr")]
    BadAddr,
}

/// 解析分享链接。单值参数取首次出现；addr 累加全部出现并保序。
pub fn parse_share_link(raw: &str) -> Result<ShareLink, ShareLinkError> {
    let (scheme, rest) = raw
        .trim()
        .split_once("://")
        .ok_or(ShareLinkError::NotShareLink)?;
    if !scheme.eq_ignore_ascii_case(SCHEME) {
        return Err(ShareLinkError::NotShareLink);
    }
    let (version, query) = match rest.split_once('?') {
        Some((v, q)) => (v, q),
        None => (rest, ""),
    };
    if !version.eq_ignore_ascii_case(VERSION) {
        return Err(ShareLinkError::BadVersion(version.to_string()));
    }
    let mut peer: Option<String> = None;
    let mut token: Option<String> = None;
    let mut exp: Option<String> = None;
    let mut sid: Option<String> = None;
    let mut addrs: Vec<String> = Vec::new();
    for (key, value) in crate::ws::parse_query(query) {
        match key.as_str() {
            "peer" if peer.is_none() => peer = Some(value),
            "token" if token.is_none() => token = Some(value),
            "addr" => addrs.push(value),
            "exp" if exp.is_none() => exp = Some(value),
            "sid" if sid.is_none() => sid = Some(value),
            _ => {}
        }
    }
    let peer = peer.ok_or(ShareLinkError::MissingPeer)?;
    let peer_id = crate::dial::parse_peer_id(&peer).map_err(ShareLinkError::BadPeer)?;
    let token = token.ok_or(ShareLinkError::MissingToken)?;
    if !is_share_token(&token) {
        return Err(ShareLinkError::BadToken);
    }
    if addrs.iter().any(String::is_empty) {
        return Err(ShareLinkError::BadAddr);
    }
    Ok(ShareLink {
        peer,
        peer_id,
        addrs,
        token,
        exp,
        sid,
    })
}

/// token 检查：恰好 32 个 hex 字符；大小写均接受、原样透传。
fn is_share_token(s: &str) -> bool {
    s.len() == TOKEN_HEX_LEN && s.bytes().all(|b| b.is_ascii_hexdigit())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn peer_of(bytes: [u8; 32]) -> String {
        PeerId::from_bytes(bytes).to_string()
    }

    fn link(peer: &str, tail: &str) -> String {
        let token = "a1".repeat(16);
        match tail {
            "" => format!("{SCHEME}://{VERSION}?peer={peer}&token={token}"),
            _ => format!("{SCHEME}://{VERSION}?peer={peer}&token={token}&{tail}"),
        }
    }

    #[test]
    fn minimal_link_parses_without_addr() {
        let link = parse_share_link(&link(&peer_of([1; 32]), "")).unwrap();
        assert_eq!(link.peer, peer_of([1; 32]));
        assert_eq!(link.peer_id, PeerId::from_bytes([1; 32]));
        assert!(link.addrs.is_empty());
        assert_eq!(link.token, "a1".repeat(16));
        assert_eq!(link.exp, None);
        assert_eq!(link.sid, None);
    }

    #[test]
    fn repeated_addrs_keep_link_order_and_hints_pass_through() {
        let peer = peer_of([2; 32]);
        let link = parse_share_link(&link(
            &peer,
            "addr=/ip4/10.0.0.8/tcp/4001&addr=/ip4/10.0.0.9/u4002&exp=123&sid=s-1",
        ))
        .unwrap();
        assert_eq!(
            link.addrs,
            vec!["/ip4/10.0.0.8/tcp/4001", "/ip4/10.0.0.9/u4002"]
        );
        assert_eq!(link.exp.as_deref(), Some("123"));
        assert_eq!(link.sid.as_deref(), Some("s-1"));
    }

    #[test]
    fn past_exp_is_hint_not_rejection_and_unknown_params_ignored() {
        let peer = peer_of([3; 32]);
        let link = parse_share_link(&link(&peer, "exp=1&sid=x&foo=bar&baz=")).unwrap();
        assert_eq!(link.exp.as_deref(), Some("1"));
    }

    #[test]
    fn percent_encoded_addr_decodes_and_upper_hex_token_passes() {
        let peer = peer_of([4; 32]);
        let raw = format!(
            "{SCHEME}://{VERSION}?peer={peer}&addr=%2Fip4%2F10.0.0.8%2Ftcp%2F4001&token={}",
            "AB".repeat(16)
        );
        let link = parse_share_link(&raw).unwrap();
        assert_eq!(link.addrs, vec!["/ip4/10.0.0.8/tcp/4001"]);
        assert_eq!(link.token, "AB".repeat(16));
    }

    #[test]
    fn single_valued_params_take_first_occurrence() {
        let peer = peer_of([5; 32]);
        let raw = format!(
            "{SCHEME}://{VERSION}?peer={peer}&peer=zz&token={}&&token=ff",
            "a1".repeat(16)
        );
        let link = parse_share_link(&raw).unwrap();
        assert_eq!(link.peer, peer);
        assert_eq!(link.token, "a1".repeat(16));
    }

    #[test]
    fn scheme_and_version_mismatch_rejected() {
        let peer = peer_of([6; 32]);
        for raw in [
            format!("https://v1?peer={peer}&token={}", "a1".repeat(16)),
            format!("dsh-acp-share2://v1?peer={peer}&token={}", "a1".repeat(16)),
            format!("{SCHEME}://v2?peer={peer}&token={}", "a1".repeat(16)),
        ] {
            assert!(parse_share_link(&raw).is_err(), "raw={raw}");
        }
    }

    #[test]
    fn missing_peer_or_token_rejected() {
        let token = "a1".repeat(16);
        assert!(matches!(
            parse_share_link(&format!("{SCHEME}://{VERSION}?token={token}")),
            Err(ShareLinkError::MissingPeer)
        ));
        assert!(matches!(
            parse_share_link(&format!("{SCHEME}://{VERSION}?peer={}", peer_of([7; 32]))),
            Err(ShareLinkError::MissingToken)
        ));
    }

    #[test]
    fn bad_peer_or_token_rejected() {
        let token = "a1".repeat(16);
        assert!(matches!(
            parse_share_link(&format!("{SCHEME}://{VERSION}?peer=!!!&token={token}")),
            Err(ShareLinkError::BadPeer(_))
        ));
        // 合法 base58 但字节数不足 32 的假 peer。
        let short = "2".repeat(32);
        assert!(matches!(
            parse_share_link(&format!("{SCHEME}://{VERSION}?peer={short}&token={token}")),
            Err(ShareLinkError::BadPeer(_))
        ));
        let peer = peer_of([8; 32]);
        assert!(matches!(
            parse_share_link(&format!("{SCHEME}://{VERSION}?peer={peer}&token=abc")),
            Err(ShareLinkError::BadToken)
        ));
        assert!(matches!(
            parse_share_link(&format!(
                "{SCHEME}://{VERSION}?peer={peer}&token={}",
                "zz".repeat(16)
            )),
            Err(ShareLinkError::BadToken)
        ));
    }

    #[test]
    fn empty_addr_rejected() {
        let peer = peer_of([9; 32]);
        assert!(matches!(
            parse_share_link(&format!(
                "{SCHEME}://{VERSION}?peer={peer}&token={}&addr=",
                "a1".repeat(16)
            )),
            Err(ShareLinkError::BadAddr)
        ));
    }

    #[test]
    fn surrounding_whitespace_trimmed() {
        let peer = peer_of([10; 32]);
        let raw = format!(" {} ", link(&peer, ""));
        assert!(parse_share_link(&raw).is_ok());
    }
}

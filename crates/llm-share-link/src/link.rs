//! 分享链接：dsh-llm-share://v1?peer=&addr=&token=&exp=&sid=&models= 的解析与组装。
//! 链接是展示性凭证（exp/sid 提示性，权威判定在出借方台账）；本模块只做形态校验。

/// 链接 scheme（对齐 ACP 分享链接先例，独立 scheme 分发）。
pub const SCHEME: &str = "dsh-llm-share";

/// 解析后的分享链接字段。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShareLink {
    pub peer: String,
    /// 候选地址（QUIC/TCP/中继，可重复）。
    pub addrs: Vec<String>,
    pub token: String,
    /// 到期提示（Unix 秒）；畸形视为缺省。
    pub exp: Option<u64>,
    pub sid: Option<String>,
    /// 模型范围；空串视为 None（缺省=不限模型在创建侧禁止，权威在出借方）。
    pub models: Option<Vec<String>>,
}

/// 链接解析错误（形态级校验，业务拒绝在兑换侧）。
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum LinkError {
    #[error("scheme 不符（期望 {SCHEME}，实际 {0}）")]
    BadScheme(String),
    #[error("链接缺少 peer 参数")]
    MissingPeer,
    #[error("链接缺少 token 参数")]
    MissingToken,
    #[error("token 非法（应恰 32 位小写 hex）：{0}")]
    BadToken(String),
}

/// token 形态校验：恰 32 位小写 hex。
pub fn is_valid_token(token: &str) -> bool {
    token.len() == 32
        && token
            .bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
}

/// 解析分享链接。scheme 不符 / peer 缺失 / token 缺失 / token 非法 → 显式错误；
/// 未知参数忽略；exp/sid 畸形视为缺省；models 空串 → None；addr 可重复。
pub fn parse_link(raw: &str) -> Result<ShareLink, LinkError> {
    let rest = raw
        .strip_prefix(&format!("{SCHEME}://"))
        .ok_or_else(|| LinkError::BadScheme(scheme_of(raw)))?;
    let query = rest.split_once('?').map(|(_, q)| q).unwrap_or("");
    let mut peer = None;
    let mut addrs = Vec::new();
    let mut token = None;
    let mut exp = None;
    let mut sid = None;
    let mut models = None;
    for pair in query.split('&') {
        if pair.is_empty() {
            continue;
        }
        let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
        match key {
            "peer" => {
                if peer.is_none() && !value.is_empty() {
                    peer = Some(value.to_owned());
                }
            }
            "addr" => {
                if !value.is_empty() {
                    addrs.push(value.to_owned());
                }
            }
            "token" => {
                if token.is_none() && !value.is_empty() {
                    token = Some(value.to_owned());
                }
            }
            "exp" => {
                if exp.is_none() {
                    exp = value.parse().ok();
                }
            }
            "sid" => {
                if sid.is_none() {
                    sid = Some(value.to_owned());
                }
            }
            "models" if models.is_none() => {
                models = if value.is_empty() {
                    None
                } else {
                    Some(value.split(',').map(str::to_owned).collect())
                };
            }
            _ => {}
        }
    }
    let peer = peer.ok_or(LinkError::MissingPeer)?;
    let token = token.ok_or(LinkError::MissingToken)?;
    if !is_valid_token(&token) {
        return Err(LinkError::BadToken(token));
    }
    Ok(ShareLink {
        peer,
        addrs,
        token,
        exp,
        sid,
        models,
    })
}

/// 组装分享链接（peer/addrs/token/exp/sid/models 全量参数）。
pub fn build_link(
    peer: &str,
    addrs: &[String],
    token: &str,
    exp: Option<u64>,
    sid: Option<String>,
    models: Option<Vec<String>>,
) -> String {
    let mut query = vec![format!("peer={peer}")];
    query.extend(addrs.iter().map(|addr| format!("addr={addr}")));
    query.push(format!("token={token}"));
    if let Some(exp) = exp {
        query.push(format!("exp={exp}"));
    }
    if let Some(sid) = sid {
        query.push(format!("sid={sid}"));
    }
    if let Some(models) = models {
        if !models.is_empty() {
            query.push(format!("models={}", models.join(",")));
        }
    }
    format!("{SCHEME}://v1?{}", query.join("&"))
}

/// 错误定位用：取原串的 scheme 段（无 :// 时整体视为 scheme）。
fn scheme_of(raw: &str) -> String {
    raw.split_once("://")
        .map(|(scheme, _)| scheme.to_owned())
        .unwrap_or_else(|| raw.to_owned())
}

#[cfg(test)]
mod tests;

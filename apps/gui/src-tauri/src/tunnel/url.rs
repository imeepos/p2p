//! DSH 启动 URL 解析（冻结契约 §6）：`http://127.0.0.1:<port>/?token=<tok>`。
//! host 只认 `127.0.0.1` 回环字面量（cookie 只看 host 不看 port，必须与重写后
//! 的 Host authority 同源）；token 走 query `token=`，容忍 %XX 转义。

/// 解析产物：DSH 端口与鉴权 token。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DshTarget {
    pub port: u16,
    pub token: String,
}

/// 解析 DSH 启动 URL；任何不规范输入显式报错（GUI 错误态直用人话）。
pub fn parse_dsh_url(raw: &str) -> Result<DshTarget, String> {
    let rest = raw
        .strip_prefix("http://")
        .ok_or_else(|| format!("URL 需以 http:// 开头: {raw}"))?;
    let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
    let (host, port_raw) = authority
        .rsplit_once(':')
        .ok_or_else(|| format!("端口缺失: {authority}"))?;
    if host != "127.0.0.1" {
        return Err(format!("host 只允许 127.0.0.1 回环字面量: {host}"));
    }
    let port: u16 = port_raw
        .parse()
        .map_err(|_| format!("端口非法: {port_raw}"))?;
    let token = extract_query_param(rest, "token")
        .filter(|t| !t.is_empty())
        .ok_or("缺少 token query 参数")?;
    Ok(DshTarget { port, token })
}

/// 取 query 参数并做 %XX/`+` 解码；找不到键返回 None。
fn extract_query_param(rest: &str, name: &str) -> Option<String> {
    let query = rest.split_once('?')?.1;
    let query = query.split('#').next().unwrap_or(query);
    query.split('&').find_map(|pair| {
        let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
        if percent_decode(key) == name {
            Some(percent_decode(value))
        } else {
            None
        }
    })
}

fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'%' if i + 3 <= bytes.len() => {
                let hex = std::str::from_utf8(&bytes[i + 1..i + 3])
                    .ok()
                    .and_then(|h| u8::from_str_radix(h, 16).ok());
                match hex {
                    Some(b) => {
                        out.push(b);
                        i += 3;
                    }
                    None => {
                        out.push(b'%');
                        i += 1;
                    }
                }
            }
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            b => {
                out.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_dsh_boot_url() {
        let target = parse_dsh_url("http://127.0.0.1:3080/?token=abc-DEF_123").expect("parse");
        assert_eq!(target.port, 3080);
        assert_eq!(target.token, "abc-DEF_123");
    }

    #[test]
    fn token_may_appear_after_other_params() {
        let target =
            parse_dsh_url("http://127.0.0.1:80/?foo=1&token=a%20b+c").expect("parse");
        assert_eq!(target.token, "a b c");
    }

    #[test]
    fn rejects_non_loopback_and_bad_input() {
        assert!(parse_dsh_url("http://localhost:3080/?token=t")
            .unwrap_err()
            .contains("127.0.0.1"));
        assert!(parse_dsh_url("https://127.0.0.1:3080/?token=t")
            .unwrap_err()
            .contains("http://"));
        assert!(parse_dsh_url("http://127.0.0.1/?token=t")
            .unwrap_err()
            .contains("端口缺失"));
        assert!(parse_dsh_url("http://127.0.0.1:70000/?token=t")
            .unwrap_err()
            .contains("端口非法"));
        assert!(parse_dsh_url("http://127.0.0.1:3080/?other=t")
            .unwrap_err()
            .contains("token"));
    }
}

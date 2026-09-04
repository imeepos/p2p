//! 运行配置：CLI 入参 → 组件装配参数。默认值取 acp-common 常量与设计 §6 本地面。

use std::path::PathBuf;
use std::time::Duration;

use acp_common::consts::REATTACH_WINDOW_DEFAULT_SECS;

#[derive(Clone, Debug)]
pub struct ConsoleConfig {
    /// 数据目录：reattach 票据落这里，P2P 身份目录 = data_dir/p2p-identity。
    pub data_dir: PathBuf,
    /// rendezvous bootstrap 地址；空则发现仅 mDNS + 手动登记。
    pub bootstrap: Vec<String>,
    /// mDNS 局域网发现开关（默认开）。
    pub mdns: bool,
    /// 手动登记候选：PeerId → 地址表（D 的手动面，直拨入口）。
    pub manual_peers: Vec<(String, Vec<String>)>,
    /// 握手 token 可选透传（A；也可由 WS 查询参数 atoken 逐连接指定）。
    pub agent_token: Option<String>,
    /// 本地 WS 绑定端口（0 = 随机）。
    pub ws_port: u16,
    /// status HTTP 绑定端口（0 = 随机）。
    pub status_port: u16,
    /// 断流后续连窗口（C；默认设计 §5 的 90 s，ACP4 续连依赖此语义）。
    pub reattach_window: Duration,
}

impl Default for ConsoleConfig {
    fn default() -> Self {
        Self {
            data_dir: PathBuf::from("./acp-console-data"),
            bootstrap: Vec::new(),
            mdns: true,
            manual_peers: Vec::new(),
            agent_token: None,
            ws_port: 0,
            status_port: 0,
            reattach_window: Duration::from_secs(REATTACH_WINDOW_DEFAULT_SECS),
        }
    }
}

/// 解析 PEER@ADDR 手动登记项：同一 PeerId 多次出现时地址聚合，保持首次出现顺序。
/// 缺 '@' 或任一侧为空即为结构化错误（启动 fail-fast，不静默丢弃）。
pub fn parse_manual_peers(specs: &[String]) -> Result<Vec<(String, Vec<String>)>, String> {
    let mut order: Vec<String> = Vec::new();
    let mut grouped: std::collections::BTreeMap<String, Vec<String>> =
        std::collections::BTreeMap::new();
    for spec in specs {
        let (peer, addr) = match spec.split_once('@') {
            Some((p, a)) if !p.is_empty() && !a.is_empty() => (p, a),
            _ => return Err(format!("bad --peer spec (want PEER@ADDR): {spec}")),
        };
        if !order.iter().any(|p| p == peer) {
            order.push(peer.to_string());
        }
        grouped
            .entry(peer.to_string())
            .or_default()
            .push(addr.to_string());
    }
    Ok(order
        .into_iter()
        .map(|peer| {
            let addrs = grouped.get(&peer).cloned().unwrap_or_default();
            (peer, addrs)
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_groups_by_peer_keeps_order() {
        let specs = vec![
            "AAA@/ip4/10.0.0.1/tcp/1".to_string(),
            "BBB@/ip4/10.0.0.2/tcp/2".to_string(),
            "AAA@/ip4/10.0.0.3/tcp/3".to_string(),
        ];
        let got = parse_manual_peers(&specs).unwrap();
        assert_eq!(got.len(), 2);
        assert_eq!(got[0].0, "AAA");
        assert_eq!(got[0].1, vec!["/ip4/10.0.0.1/tcp/1", "/ip4/10.0.0.3/tcp/3"]);
        assert_eq!(got[1].0, "BBB");
    }

    #[test]
    fn parse_rejects_malformed_spec() {
        assert!(parse_manual_peers(&["no-at-sign".to_string()]).is_err());
        assert!(parse_manual_peers(&["@/ip4/0.0.0.0/tcp/1".to_string()]).is_err());
        assert!(parse_manual_peers(&["peer@".to_string()]).is_err());
    }
}

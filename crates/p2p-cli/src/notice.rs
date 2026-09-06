//! 启动外联声明（F8）：node start/serve 装配前输出网络模式事实源，
//! 人读文本与 --json 两形态同源生成；公网端点显式可感知、lan-only 可缺席验证。

use serde_json::{json, Value};

/// 公共设施一类别：bootstrap（rendezvous 发现）/ relay（中继兜底）/ observation（地址观测）。
pub struct PublicEndpoint {
    pub category: &'static str,
    pub purpose: &'static str,
    pub addrs: Vec<String>,
}

/// 网络外联声明：lan_only=true 时 endpoints 必为空（仅局域网，无公网端点可列）。
pub struct NetworkNotice {
    pub lan_only: bool,
    pub endpoints: Vec<PublicEndpoint>,
}

impl NetworkNotice {
    pub fn lan_only() -> Self {
        Self {
            lan_only: true,
            endpoints: Vec::new(),
        }
    }

    pub fn public(endpoints: Vec<PublicEndpoint>) -> Self {
        Self {
            lan_only: false,
            endpoints,
        }
    }

    /// 人读形态：lan-only 一行声明；公网模式逐类列出「类别: 意图(地址)」。
    pub fn text(&self) -> String {
        if self.lan_only {
            return "网络模式: 仅局域网(lan-only), 不连接任何公共设施(无公网外联)".into();
        }
        let mut lines = vec!["网络模式: 将连接以下公共设施(公网外联)".into()];
        for ep in &self.endpoints {
            lines.push(format!(
                "- {}: {} [{}]",
                ep.category,
                ep.purpose,
                ep.addrs.join(", ")
            ));
        }
        lines.join("\n")
    }

    /// JSON 形态：lanOnly + endpoints 同源（camelCase，对齐 p2pctl --json 约定）。
    pub fn json(&self) -> Value {
        json!({
            "lanOnly": self.lan_only,
            "endpoints": self
                .endpoints
                .iter()
                .map(|ep| {
                    json!({
                        "category": ep.category,
                        "purpose": ep.purpose,
                        "addrs": ep.addrs,
                    })
                })
                .collect::<Vec<_>>(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_endpoints() -> Vec<PublicEndpoint> {
        vec![
            PublicEndpoint {
                category: "bootstrap",
                purpose: "rendezvous 跨网发现注册与查号",
                addrs: vec!["43.240.223.138/u3400".into()],
            },
            PublicEndpoint {
                category: "relay",
                purpose: "打洞失败后的中继兜底",
                addrs: vec!["43.240.223.138/u3403".into()],
            },
            PublicEndpoint {
                category: "observation",
                purpose: "学习自身公网映射地址",
                addrs: vec!["121.196.193.177:3402".into()],
            },
        ]
    }

    #[test]
    fn notice_lan_only_declares_lan_and_omits_endpoints() {
        let notice = NetworkNotice::lan_only();
        let text = notice.text();
        assert!(text.contains("仅局域网"), "lan-only 声明缺席: {text}");
        assert!(text.contains("lan-only"), "关键字 lan-only 缺席: {text}");
        assert!(
            !text.contains("43.240.223.138"),
            "lan-only 不得列出公网端点: {text}"
        );
        let v = notice.json();
        assert_eq!(v["lanOnly"], json!(true));
        assert_eq!(
            v["endpoints"].as_array().map(Vec::len),
            Some(0),
            "lan-only JSON 端点集必须为空"
        );
    }

    #[test]
    fn notice_public_mode_lists_each_category_in_both_shapes() {
        let notice = NetworkNotice::public(sample_endpoints());
        let text = notice.text();
        for (category, purpose) in [
            ("bootstrap", "rendezvous"),
            ("relay", "中继兜底"),
            ("observation", "公网映射地址"),
        ] {
            assert!(text.contains(category), "类别 {category} 缺席: {text}");
            assert!(text.contains(purpose), "意图 {purpose} 缺席: {text}");
        }
        let v = notice.json();
        assert_eq!(v["lanOnly"], json!(false));
        let endpoints = v["endpoints"].as_array().expect("endpoints array");
        assert_eq!(endpoints.len(), 3);
        assert_eq!(endpoints[0]["category"], json!("bootstrap"));
        assert_eq!(endpoints[2]["addrs"][0], json!("121.196.193.177:3402"));
    }

    #[test]
    fn notice_empty_public_endpoints_still_declares_mode() {
        let notice = NetworkNotice::public(Vec::new());
        assert!(notice.text().contains("公共设施"), "空端点仍需模式声明");
        assert_eq!(notice.json()["lanOnly"], json!(false));
    }
}

//! 绑定模型（authz-role-design §3）：peer → 恰一个角色；expires_at 为 Unix 秒。
//! 过期判定含到期秒：now_unix >= expires_at 即视为已过（默认拒绝偏严，见 expired_at）。

use serde::{Deserialize, Serialize};

/// peer 与角色的绑定条目；单值语义 = 同 peer 重复 bind 即 upsert。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Binding {
    pub peer_id: String,
    pub role_id: String,
    /// 授予时刻（Unix 秒），由 Clock 注入。
    pub granted_at: u64,
    #[serde(default)]
    pub note: String,
    /// 授权到期（Unix 秒）；None = 不过期。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<u64>,
}

impl Binding {
    /// 到期判定：含到期秒边界（expires_at 当秒即拒，偏严不偏宽）。
    pub fn expired_at(&self, now_unix: u64) -> bool {
        self.expires_at.is_some_and(|exp| now_unix >= exp)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn binding(expires_at: Option<u64>) -> Binding {
        Binding {
            peer_id: "peer-a".to_owned(),
            role_id: "friend".to_owned(),
            granted_at: 1_000,
            note: String::new(),
            expires_at,
        }
    }

    #[test]
    fn expiry_boundary_is_inclusive() {
        let b = binding(Some(2_000));
        assert!(!b.expired_at(1_999), "到期前一秒仍有效");
        assert!(b.expired_at(2_000), "到期秒含边界即拒（偏严）");
        assert!(b.expired_at(2_001));
    }

    #[test]
    fn no_expiry_never_expires() {
        assert!(!binding(None).expired_at(u64::MAX));
    }

    #[test]
    fn serde_roundtrip_omits_absent_expiry() {
        let text = serde_json::to_string(&binding(Some(2_000))).unwrap();
        assert!(text.contains("expires_at"));
        let parsed: Binding = serde_json::from_str(&text).unwrap();
        assert_eq!(parsed, binding(Some(2_000)));

        let no_exp = serde_json::to_string(&binding(None)).unwrap();
        assert!(!no_exp.contains("expires_at"));
        let parsed: Binding = serde_json::from_str(&no_exp).unwrap();
        assert_eq!(parsed.expires_at, None);
    }
}

//! 权限登记表（authz-role-design §4）：首批闭集九 key，单一真值源。
//! key 形如 "<domain>.<capability>"，只加不删（废用标 deprecated，不删除）；
//! registry 无 owner-only key，模型上杜绝经角色提权到 owner（§11 红线 1）。
//! 数据一致性测试逐字锚定本表与内建角色阶梯（§5），改表即测试红。

use std::fmt;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// 闭集权限 key。构造仅限本表常量与 [Permission::parse]（表外字符串进不来）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Permission(&'static str);

impl Permission {
    pub const CHAT_SEND: Permission = Permission("chat.send");
    pub const CHAT_ATTACHMENT: Permission = Permission("chat.attachment");
    pub const A2A_DISCOVER: Permission = Permission("a2a.discover");
    pub const A2A_INVOKE: Permission = Permission("a2a.invoke");
    pub const ACP_SESSION: Permission = Permission("acp.session");
    pub const ACP_EXECUTE: Permission = Permission("acp.execute");
    pub const LLM_BORROW: Permission = Permission("llm.borrow");
    pub const REPAIR_DIAG: Permission = Permission("repair.diag");
    pub const REPAIR_FIX: Permission = Permission("repair.fix");

    pub const fn as_str(self) -> &'static str {
        self.0
    }

    /// 闭集精确匹配解析：表外 key 一律 None（自定义角色子集的唯一入口）。
    pub fn parse(key: &str) -> Option<Self> {
        REGISTRY.iter().copied().find(|p| p.0 == key)
    }

    pub fn registry() -> &'static [Self] {
        REGISTRY
    }
}

/// §4 首批闭集（九 key），顺序与设计表一致。
static REGISTRY: &[Permission] = &[
    Permission::CHAT_SEND,
    Permission::CHAT_ATTACHMENT,
    Permission::A2A_DISCOVER,
    Permission::A2A_INVOKE,
    Permission::ACP_SESSION,
    Permission::ACP_EXECUTE,
    Permission::LLM_BORROW,
    Permission::REPAIR_DIAG,
    Permission::REPAIR_FIX,
];

impl fmt::Display for Permission {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.0)
    }
}

impl Serialize for Permission {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.0)
    }
}

impl<'de> Deserialize<'de> for Permission {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let key = String::deserialize(deserializer)?;
        Permission::parse(&key)
            .ok_or_else(|| serde::de::Error::custom(format!("权限 key 不在闭集登记表: {key}")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// §4 逐字锚点：改表即测试红。
    const DESIGN_KEYS: [&str; 9] = [
        "chat.send",
        "chat.attachment",
        "a2a.discover",
        "a2a.invoke",
        "acp.session",
        "acp.execute",
        "llm.borrow",
        "repair.diag",
        "repair.fix",
    ];

    #[test]
    fn registry_is_nine_keys_verbatim() {
        assert_eq!(REGISTRY.len(), 9);
        let keys: Vec<_> = REGISTRY.iter().map(|p| p.as_str()).collect();
        assert_eq!(keys, DESIGN_KEYS);
    }

    #[test]
    fn parse_covers_registry_and_rejects_outside_keys() {
        for key in DESIGN_KEYS {
            let perm = Permission::parse(key).unwrap_or_else(|| panic!("表内 key 解析失败: {key}"));
            assert_eq!(perm.as_str(), key);
        }
        assert_eq!(Permission::parse("owner.superuser"), None);
        assert_eq!(Permission::parse(""), None);
        assert_eq!(Permission::parse("chat.send.extra"), None);
        assert_eq!(Permission::parse("Chat.Send"), None);
    }

    #[test]
    fn serde_roundtrip_and_unknown_key_rejected() {
        let json = serde_json::to_string(&Permission::LLM_BORROW).unwrap();
        assert_eq!(json, "\"llm.borrow\"");
        let parsed: Permission = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, Permission::LLM_BORROW);
        let err = serde_json::from_str::<Permission>("\"owner.superuser\"");
        assert!(err.is_err(), "表外 key 必须反序列化失败（闭集）");
    }
}

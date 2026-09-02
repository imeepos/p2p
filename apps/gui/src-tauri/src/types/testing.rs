//! 契约类型测试助手（roundtrip 与样例配置）。

use super::GuiConfig;
use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::Value;

/// 编码结果必须与契约 JSON 逐字段一致，且能从契约 JSON 原样还原。
pub(crate) fn roundtrip<T>(value: &T, raw: Value)
where
    T: Serialize + DeserializeOwned + PartialEq + std::fmt::Debug,
{
    let encoded = serde_json::to_value(value).expect("序列化");
    assert_eq!(encoded, raw, "序列化字段与契约不一致");
    let decoded: T = serde_json::from_value(raw).expect("反序列化");
    assert_eq!(&decoded, value, "roundtrip 不保真");
}

/// 与云端端点无关的样例配置（serde 断言用假地址）。
pub(crate) fn sample_config() -> GuiConfig {
    GuiConfig {
        quic_port: 3400,
        tcp_port: 3401,
        enable_mdns: true,
        data_dir: "/data/p2p-data".into(),
        bootstrap: vec!["1.2.3.4/u3400".into(), "1.2.3.4/t3401".into()],
        relay_addrs: vec!["5.6.7.8/u3400".into()],
        advertised_addrs: vec!["9.9.9.9/u4000".into()],
        observation_port: Some(3402),
        observation_addrs: vec!["1.2.3.4:3402".into()],
    }
}

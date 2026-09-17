//! ConfigStore 持久化单测：出厂默认契约、部分配置补默认（红绿双向）、roundtrip。
//! 自 config.rs 拆出：该文件超 300 行红线，测试居子文件（types/tests.rs 先例）。

use std::fs;
use std::path::PathBuf;

use serde_json::json;

use super::*;

/// 独立临时目录：测试间互不污染，结束清理。
fn temp_root(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("p2p-console-{}-{}", tag, std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("创建临时目录");
    dir
}

#[test]
fn default_config_matches_contract() {
    let dir = temp_root("default");
    let store = ConfigStore::new(dir.join("app"));
    let cfg = store.default_config();
    assert_eq!(cfg.quic_port, 0);
    assert_eq!(cfg.tcp_port, 0);
    assert!(cfg.enable_mdns);
    assert_eq!(
        cfg.data_dir,
        dir.join("app").join("p2p-data").to_string_lossy()
    );
    assert_eq!(
        cfg.bootstrap,
        vec!["43.240.223.138/u3400", "121.196.193.177/u3400"]
    );
    assert_eq!(
        cfg.relay_addrs,
        vec!["43.240.223.138/u3403", "121.196.193.177/u3403"]
    );
    assert!(cfg.advertised_addrs.is_empty());
    assert_eq!(cfg.observation_port, None);
    assert_eq!(cfg.observation_addrs, vec!["121.196.193.177:3402"]);
    // CC2 三字段出厂缺省：true / 15 / 空
    assert!(cfg.rd_require_approval);
    assert_eq!(cfg.rd_fps, 15);
    assert!(cfg.tunnel_serve_allow.is_empty());
    let raw = serde_json::to_value(&cfg).unwrap();
    assert_eq!(raw["rdRequireApproval"], json!(true), "camelCase 契约键名");
    assert_eq!(raw["rdFps"], json!(15));
    assert_eq!(raw["tunnelServeAllow"], json!([]));
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn missing_file_loads_default() {
    let dir = temp_root("missing");
    let store = ConfigStore::new(dir.join("app"));
    assert_eq!(store.load(), store.default_config());
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn save_load_roundtrip_is_atomic_and_stable() {
    let dir = temp_root("roundtrip");
    let store = ConfigStore::new(dir.join("app"));
    let mut cfg = store.default_config();
    cfg.quic_port = 3400;
    cfg.tcp_port = 3401;
    cfg.bootstrap = vec!["1.2.3.4/3400".into(), "5.6.7.8/t3401".into()];
    cfg.relay_addrs = vec!["1.2.3.4/3400".into()];
    cfg.advertised_addrs = vec!["9.9.9.9/4000".into()];
    cfg.observation_port = Some(3402);
    cfg.observation_addrs = vec!["1.2.3.4:3402".into()];
    cfg.rd_require_approval = false;
    cfg.rd_fps = 30;
    cfg.tunnel_serve_allow = vec!["127.0.0.1:5900".into()];
    store.save(&cfg).expect("保存配置");
    // 临时文件已被 rename 消费，不留残骸
    assert!(!dir.join("app").join("gui-config.json.tmp").exists());
    assert_eq!(store.load(), cfg, "CC2 三字段显式值 roundtrip 保真");
    // 二次保存覆盖旧值
    cfg.quic_port = 0;
    store.save(&cfg).expect("再次保存");
    assert_eq!(store.load(), cfg);
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn partial_config_fills_missing_fields_with_defaults() {
    let dir = temp_root("partial");
    let app = dir.join("app");
    fs::create_dir_all(&app).expect("创建 app 目录");
    // 旧版配置：只有用户改过的字段，缺端点/端口字段；rdFps 显式 30 证显式值保留
    fs::write(
        app.join("gui-config.json"),
        json!({ "enableMdns": false, "quicPort": 3400, "rdFps": 30 }).to_string(),
    )
    .expect("写入部分配置");
    let store = ConfigStore::new(app.clone());
    let cfg = store.load();
    assert!(!cfg.enable_mdns, "用户已有字段不得被默认覆盖");
    assert_eq!(cfg.quic_port, 3400, "用户已有字段不得被默认覆盖");
    assert_eq!(
        cfg.bootstrap,
        crate::config::default_bootstrap(),
        "缺失字段应补出厂默认端点"
    );
    assert_eq!(cfg.relay_addrs, crate::config::default_relay_addrs());
    assert_eq!(
        cfg.observation_addrs,
        crate::config::default_observation_addrs()
    );
    assert_eq!(cfg.tcp_port, 0);
    assert!(!cfg.lan_only, "旧配置缺 lanOnly 字段补缺省 false（§16.5）");
    // CC2 双向：缺字段补缺省（红），显式值不被覆盖（绿）
    assert!(
        cfg.rd_require_approval,
        "旧配置缺 rdRequireApproval 补缺省 true"
    );
    assert_eq!(cfg.rd_fps, 30, "用户显式 rdFps 不得被默认覆盖");
    assert!(
        cfg.tunnel_serve_allow.is_empty(),
        "旧配置缺 tunnelServeAllow 补缺省空"
    );
    let _ = fs::remove_dir_all(&dir);
}

/// lanOnly 断链修复回归：设置页开关保存后重启仍生效（plan §1 B1 验收）。
#[test]
fn lan_only_survives_save_load_restart_cycle() {
    let dir = temp_root("lan-only");
    let store = ConfigStore::new(dir.join("app"));
    let mut cfg = store.default_config();
    assert!(!cfg.lan_only, "出厂默认 false");
    cfg.lan_only = true;
    store.save(&cfg).expect("保存 lanOnly=true");
    // 重启 = 重新 load（新 ConfigStore 实例模拟进程重启）
    let reloaded = ConfigStore::new(dir.join("app")).load();
    assert!(reloaded.lan_only, "lanOnly 开关保存后重启仍生效");
    let _ = fs::remove_dir_all(&dir);
}

/// CC2：serve 白名单写通后重启仍可恢复（持久化语义，区别于会话态开关）。
#[test]
fn tunnel_serve_allow_survives_restart() {
    let dir = temp_root("serve-allow");
    let store = ConfigStore::new(dir.join("app"));
    let mut cfg = store.default_config();
    cfg.tunnel_serve_allow = vec!["127.0.0.1:5900".into(), "127.0.0.1:3389".into()];
    store.save(&cfg).expect("保存白名单");
    let reloaded = ConfigStore::new(dir.join("app")).load();
    assert_eq!(
        reloaded.tunnel_serve_allow,
        vec!["127.0.0.1:5900".to_owned(), "127.0.0.1:3389".to_owned()]
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn user_explicit_empty_lists_not_overridden_by_defaults() {
    let dir = temp_root("explicit-empty");
    let store = ConfigStore::new(dir.join("app"));
    let mut cfg = store.default_config();
    cfg.bootstrap = Vec::new();
    cfg.observation_addrs = Vec::new();
    store.save(&cfg).expect("保存用户显式空列表");
    let loaded = store.load();
    assert!(loaded.bootstrap.is_empty(), "用户显式空列表不得补默认");
    assert!(loaded.observation_addrs.is_empty());
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn corrupted_file_falls_back_to_default_with_warning() {
    let dir = temp_root("corrupt");
    let app = dir.join("app");
    fs::create_dir_all(&app).expect("创建 app 目录");
    fs::write(app.join("gui-config.json"), "{ not json").expect("写入坏文件");
    let store = ConfigStore::new(app);
    assert_eq!(store.load(), store.default_config());
    let _ = fs::remove_dir_all(&dir);
}

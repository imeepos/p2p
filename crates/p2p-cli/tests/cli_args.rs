//! 参数解析门禁：--bootstrap 多值与 --relay 接线（多引导面/降级链的 CLI 入口）。

use clap::Parser;
use p2p_cli::cli::{Cli, Command};

#[test]
fn node_accepts_multiple_bootstrap_and_relay() {
    let a = [
        "p2p-cli",
        "node",
        "--data",
        "d",
        "--bootstrap",
        "43.240.223.138/u3400",
        "--bootstrap",
        "121.196.193.177/u3400",
        "--relay",
        "43.240.223.138/u3403",
        "--relay",
        "121.196.193.177/u3403",
    ];
    let Command::Node(args) = Cli::try_parse_from(a).unwrap().command else {
        panic!("node");
    };
    assert_eq!(
        args.bootstrap,
        ["43.240.223.138/u3400", "121.196.193.177/u3400"]
    );
    assert_eq!(
        args.relay,
        ["43.240.223.138/u3403", "121.196.193.177/u3403"]
    );
}

#[test]
fn ping_accepts_multiple_bootstrap_and_relay() {
    let a = [
        "p2p-cli",
        "ping",
        "abc",
        "--bootstrap",
        "1.1.1.1/u3400",
        "--bootstrap",
        "2.2.2.2/u3400",
        "--relay",
        "1.1.1.1/u3403",
    ];
    let Command::Ping(args) = Cli::try_parse_from(a).unwrap().command else {
        panic!("ping");
    };
    assert_eq!(args.bootstrap.len(), 2);
    assert_eq!(args.relay, ["1.1.1.1/u3403"]);
}

#[test]
fn discover_accepts_multiple_bootstrap() {
    let a = [
        "p2p-cli",
        "discover",
        "--bootstrap",
        "1.1.1.1/u3400",
        "--bootstrap",
        "2.2.2.2/u3400",
    ];
    let Command::Discover(args) = Cli::try_parse_from(a).unwrap().command else {
        panic!("discover");
    };
    assert_eq!(args.bootstrap.len(), 2);
}

#[test]
fn node_bootstrap_and_relay_default_empty() {
    let Command::Node(args) = Cli::try_parse_from(["p2p-cli", "node", "--data", "d"])
        .unwrap()
        .command
    else {
        panic!("node");
    };
    assert!(args.bootstrap.is_empty());
    assert!(args.relay.is_empty());
}

#[test]
fn ping_observation_and_request_timeout_parse() {
    // E5：地址卫生过滤下 ping 必须能带观测反射；黑洞直连需可调请求预算
    let args = crate::Cli::try_parse_from([
        "p2p-cli",
        "ping",
        "APzebna1TYjK8WNA6gWbDAD6SuBTnvMbk4fw2FFSht91",
        "--bootstrap",
        "1.1.1.1/u3400",
        "--observation",
        "121.196.193.177:3402",
        "--request-timeout",
        "45",
    ])
    .expect("parse ping with observation");
    let crate::Command::Ping(ping) = args.command else {
        panic!("expected ping command");
    };
    assert_eq!(ping.observation, ["121.196.193.177:3402"]);
    assert_eq!(ping.request_timeout, 45);
}

#[test]
fn ping_request_timeout_defaults_to_twenty() {
    let args = crate::Cli::try_parse_from([
        "p2p-cli",
        "ping",
        "APzebna1TYjK8WNA6gWbDAD6SuBTnvMbk4fw2FFSht91",
        "--bootstrap",
        "1.1.1.1/u3400",
    ])
    .expect("parse default ping");
    let crate::Command::Ping(ping) = args.command else {
        panic!("expected ping command");
    };
    assert_eq!(ping.request_timeout, 20);
    assert!(ping.observation.is_empty());
}

#[test]
fn metrics_args_parse_defaults_and_overrides() {
    // E8-M2：metrics 观测入口的默认端口对齐部署约定（bootstrap +3），duration 0 常驻
    let cli =
        Cli::try_parse_from(["p2p-cli", "metrics", "--data", "d"]).expect("parse metrics defaults");
    let Command::Metrics(args) = cli.command else {
        panic!("expected metrics command");
    };
    assert_eq!(args.listen_quic, "0.0.0.0:3403");
    assert_eq!(args.listen_tcp, "0.0.0.0:3404");
    assert_eq!(args.interval, 10);
    assert_eq!(args.duration, 0);

    let cli = Cli::try_parse_from([
        "p2p-cli",
        "metrics",
        "--data",
        "d",
        "--listen-quic",
        "127.0.0.1:13403",
        "--listen-tcp",
        "127.0.0.1:13404",
        "--interval",
        "1",
        "--duration",
        "2",
    ])
    .expect("parse metrics overrides");
    let Command::Metrics(args) = cli.command else {
        panic!("expected metrics command");
    };
    assert_eq!(args.listen_quic, "127.0.0.1:13403");
    assert_eq!(args.listen_tcp, "127.0.0.1:13404");
    assert_eq!(args.interval, 1);
    assert_eq!(args.duration, 2);
}

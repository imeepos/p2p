//! 互操作：p2p-relay 消息编解码（外部消费者视角）+ RelayLimits 配额行为。
//! 未知电路、超配额路径必须给出显式错误信号，不允许静默通过。

use std::time::Duration;

use p2p_relay::messages::{read_msg, write_msg};
use p2p_relay::{errcode, CircuitId, RelayError, RelayLimits, RelayMsg};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use p2p_itest::{expect_within, relay_pair};

const LIMIT: Duration = Duration::from_secs(10);

fn all_kinds() -> Vec<RelayMsg> {
    vec![
        RelayMsg::reserve(300),
        RelayMsg::reserved(7),
        RelayMsg::connect(7),
        RelayMsg::bound(7),
        RelayMsg::punch_req("peer-b", vec!["10.0.0.1:4000".into()]),
        RelayMsg::punch_ack("peer-a", vec!["10.0.0.2:5000".into()]),
        RelayMsg::error(errcode::PEER_LIMIT, "quota"),
    ]
}

#[tokio::test]
async fn every_message_kind_roundtrips_over_duplex() {
    for msg in all_kinds() {
        let (mut tx, mut rx) = tokio::io::duplex(1024);
        write_msg(&mut tx, &msg).await.expect("write_msg");
        let got = read_msg(&mut rx)
            .await
            .expect("read_msg io")
            .expect("exactly one frame");
        assert_eq!(got, msg, "kind must survive codec roundtrip");
    }
}

#[tokio::test]
async fn corrupt_body_fails_decode_explicitly() {
    let (mut tx, mut rx) = tokio::io::duplex(64);
    // 合法 varint 帧长 + 非法 protobuf 字段键（wire type 7 不存在）
    tx.write_all(&[0x02, 0xff, 0xff]).await.expect("raw write");
    let err = read_msg(&mut rx)
        .await
        .expect_err("corrupt frame must surface as error");
    assert_eq!(err.kind(), std::io::ErrorKind::InvalidData, "got {err}");
}

#[tokio::test]
async fn unknown_circuit_connect_rejected_explicitly() {
    let (mut a, mut b) = relay_pair(RelayLimits::default(), "itest-a", "itest-b");
    let cid = expect_within("reserve", a.reserve(Duration::from_secs(60)), LIMIT)
        .await
        .expect("reserve must succeed");

    let outcome =
        expect_within("connect unknown circuit", b.connect(CircuitId(987_654)), LIMIT).await;
    match outcome {
        Err(RelayError::Server { code, message }) => {
            assert_eq!(code, errcode::UNKNOWN_CIRCUIT, "got {message}");
        }
        Ok(_) => panic!("unknown circuit must be rejected"),
        Err(other) => panic!("expected Server reject frame, got {other:?}"),
    }

    // 拒绝不留脏状态：同号正常接入仍可互通
    let (sa, sb) = expect_within(
        "valid connect",
        async { tokio::join!(a.connect(cid), b.connect(cid)) },
        LIMIT,
    )
    .await;
    let (mut sa, mut sb) = (sa.expect("a connect"), sb.expect("b connect"));
    sa.write_all(b"probe").await.expect("pump write");
    let mut got = [0u8; 5];
    sb.read_exact(&mut got).await.expect("pump read");
    assert_eq!(&got, b"probe");
}

#[tokio::test]
async fn per_peer_circuit_quota_rejects_with_peer_limit() {
    let limits = RelayLimits { max_circuits_per_peer: 1, ..RelayLimits::default() };
    let (mut a, _b) = relay_pair(limits, "quota-a", "quota-b");

    expect_within("first reserve", a.reserve(Duration::from_secs(60)), LIMIT)
        .await
        .expect("first reserve within quota");
    let outcome = expect_within("second reserve", a.reserve(Duration::from_secs(60)), LIMIT).await;
    match outcome {
        Err(RelayError::Server { code, message }) => {
            assert_eq!(code, errcode::PEER_LIMIT, "got {message}");
        }
        Ok(_) => panic!("circuit quota must cap reservations"),
        Err(other) => panic!("expected Server reject frame, got {other:?}"),
    }
}

#[tokio::test]
async fn egress_quota_write_fails_explicitly() {
    let (tx, _rx) = tokio::io::duplex(4096);
    let bucket = std::sync::Arc::new(std::sync::Mutex::new(p2p_relay::RateBucket::new(512, 512)));
    let mut limited = p2p_relay::RateLimitedStream::new(Box::new(tx), bucket);

    limited.write_all(&[0u8; 256]).await.expect("within burst must pass");
    let err = limited
        .write_all(&[0u8; 1024])
        .await
        .expect_err("beyond burst must fail explicitly");
    assert_eq!(err.kind(), std::io::ErrorKind::WriteZero, "got {err}");
}

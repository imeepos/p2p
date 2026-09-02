//! 回归：空闲连接上第二次 open_stream 必须可用（S 实测悬挂场景）。

use std::time::Duration;

use p2p_mux::{BoxedStream, MuxControl, YamuxMux, MAX_STREAMS_PER_CONN};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[tokio::test]
async fn second_open_after_idle_still_works() {
    let (a, b) = tokio::io::duplex(64 * 1024);
    let mux_a: BoxedStream = Box::new(a);
    let mux_a = YamuxMux::new(mux_a, true, MAX_STREAMS_PER_CONN);
    let mux_b = YamuxMux::new(Box::new(b), false, MAX_STREAMS_PER_CONN);

    // 第一次开流收发
    let mut s1 = mux_a.open_stream().await.expect("first open");
    s1.write_all(b"one").await.expect("write one");
    let mut r1 = mux_b.accept_stream().await.expect("first accept");
    let mut buf = [0u8; 3];
    r1.read_exact(&mut buf).await.expect("read one");
    assert_eq!(&buf, b"one");

    // 闲置 2 秒
    tokio::time::sleep(Duration::from_secs(2)).await;

    // 第二次开流必须仍可收发
    let open = tokio::time::timeout(Duration::from_secs(5), mux_a.open_stream()).await;
    let mut s2 = open
        .expect("second open must not hang")
        .expect("second open ok");
    s2.write_all(b"two").await.expect("write two");
    let accept = tokio::time::timeout(Duration::from_secs(5), mux_b.accept_stream()).await;
    let mut r2 = accept
        .expect("second accept must not hang")
        .expect("second accept ok");
    let mut buf2 = [0u8; 3];
    r2.read_exact(&mut buf2).await.expect("read two");
    assert_eq!(&buf2, b"two");
}

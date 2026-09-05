use p2p::Node;
use p2p_chat::{sanitize_name, validate_media, validate_text, Chat, ChatKind, MAX_MESSAGE_SIZE};
use std::sync::Arc;

#[test]
fn text_boundaries_trim_unicode_and_length() {
    assert_eq!(validate_text("  你好　").unwrap(), "你好");
    assert!(
        validate_text(
            " 	
"
        )
        .is_err(),
        "whitespace-only text must fail"
    );
    assert_eq!(
        validate_text(&"界".repeat(2000)).unwrap().chars().count(),
        2000
    );
    assert!(
        validate_text(&"界".repeat(2001)).is_err(),
        "2001 Unicode chars must fail"
    );
}

#[tokio::test]
async fn nickname_boundaries_trim_count_and_empty_value() {
    let dir = std::env::temp_dir().join(format!("p2p-chat-t34-nick-{}", uuid::Uuid::new_v4()));
    let node = Arc::new(
        Node::builder()
            .mdns(false)
            .quic_port(0)
            .tcp_port(0)
            .data_dir(dir.join("node"))
            .build()
            .await
            .expect("ephemeral node builds"),
    );
    let chat = Chat::new(node.clone(), dir.clone()).expect("chat store builds");
    let peer = p2p_identity::Keypair::generate().peer_id().to_string();
    assert_eq!(
        chat.friend_add(&peer, "  名字  ", vec![], None)
            .expect("trim nickname")
            .nickname,
        "名字"
    );
    assert_eq!(
        chat.friend_add(&peer, &"名".repeat(64), vec![], None)
            .expect("64 chars")
            .nickname
            .chars()
            .count(),
        64
    );
    assert!(
        chat.friend_add(&peer, &"名".repeat(65), vec![], None)
            .is_err(),
        "65 chars must fail"
    );
    assert_eq!(
        chat.friend_add(&peer, "", vec![], None)
            .expect("empty nickname allowed")
            .nickname,
        ""
    );
    node.shutdown();
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn filename_sanitization_covers_hostile_names() {
    assert_eq!(sanitize_name("../../etc/passwd"), "....etcpasswd");
    assert_eq!(sanitize_name(r"..\Windows\system32"), "..Windowssystem32");
    assert_eq!(sanitize_name("a\u{0}b\u{1f}c"), "abc");
    assert_eq!(sanitize_name("中文文件.txt"), ".txt");
    assert_eq!(sanitize_name("..."), "attachment");
    assert_eq!(sanitize_name(""), "attachment");
    assert_eq!(sanitize_name(&"x".repeat(200)).len(), 128);
}

#[test]
fn media_kind_mime_matrix_and_size_boundaries() {
    let valid = [
        (ChatKind::Image, "image/png"),
        (ChatKind::Image, "image/jpeg"),
        (ChatKind::Image, "image/gif"),
        (ChatKind::Image, "image/webp"),
        (ChatKind::Audio, "audio/mpeg"),
        (ChatKind::Audio, "audio/wav"),
        (ChatKind::Audio, "audio/ogg"),
        (ChatKind::Audio, "audio/m4a"),
        (ChatKind::Audio, "audio/mp4"),
        (ChatKind::Video, "video/mp4"),
        (ChatKind::Video, "video/webm"),
        (ChatKind::Video, "video/mov"),
        (ChatKind::Video, "video/quicktime"),
        (ChatKind::File, "application/octet-stream"),
        (ChatKind::File, "text/plain"),
    ];
    for (kind, mime) in valid {
        assert!(
            validate_media(&kind, mime, 1).is_ok(),
            "allowed MIME {kind} {mime}"
        );
    }
    for kind in [ChatKind::Image, ChatKind::Audio, ChatKind::Video] {
        assert!(
            validate_media(&kind, "application/octet-stream", 1).is_err(),
            "wrong kind MIME"
        );
    }
    assert!(
        validate_media(&ChatKind::Image, "IMAGE/PNG", 1).is_ok(),
        "MIME is case-normalized"
    );
    assert!(
        validate_media(&ChatKind::File, "application/octet-stream", 0).is_err(),
        "empty media must fail"
    );
    assert!(
        validate_media(
            &ChatKind::File,
            "application/octet-stream",
            MAX_MESSAGE_SIZE
        )
        .is_ok(),
        "64MiB is allowed"
    );
    let err = validate_media(
        &ChatKind::File,
        "application/octet-stream",
        MAX_MESSAGE_SIZE + 1,
    )
    .expect_err("64MiB+1 must fail");
    assert!(err.to_string().contains("64MiB"), "oversize signal: {err}");
}

//! rd-wire：远程桌面线协议纯逻辑 crate（M1，平台无关）。
//!
//! 协议 ID（docs/design/remote-desktop-plan.md §2.2/§3）：
//! - [/rd/control/1]：JSON 控制帧（握手/输入/剪贴板/显示/质量/心跳/关闭），双向
//! - [/rd/video/1]：二进制视频帧信封（帧头 + 载荷，chunked 承载大帧），host→viewer
//! - [/rd/file/1]：JSON 文件控制 + base64 数据块，双向
//! - `/rd/audio/1`：预留信道，注册表 planned，代码字面量不预埋（protocol-registry 门禁）
//!
//! 本 crate 只做消息模型与编解码；真实接入由 rd-host/rd-viewer 装配（后续里程碑）。
//! 帧封装复用 p2p-protocol（varint 长度前缀 ≤1 MiB；chunked 分块 ≤64 MiB 防御上限）。

mod control;
mod file;
mod io;
mod video;

pub use control::{validate as validate_control, Caps, ControlMsg, DisplayInfo, Role};
pub use file::{sanitize_rel_path, Entry, FileMsg, FsKind, XferDir, MAX_DATA_RAW_BYTES};
pub use io::{recv_control, recv_file, recv_large, send_control, send_file, send_large};
pub use video::{decode_frame, encode_frame, FrameHeader, Rect};

/// 控制通道协议 ID（心办法：与 registry.toml / specs / wire-protocol.md §3.2 四向一致）。
pub const CONTROL_PROTOCOL_ID: &str = "/rd/control/1";
/// 视频通道协议 ID。
pub const VIDEO_PROTOCOL_ID: &str = "/rd/video/1";
/// 文件传输通道协议 ID。
pub const FILE_PROTOCOL_ID: &str = "/rd/file/1";

/// 协议版本：hello.v 与视频信封 ver 字段共用。
pub const PROTOCOL_VERSION: u8 = 1;

/// rd-wire 编解码错误：解码端对任意非法输入 MUST 报错拒绝，不猜测不降级。
#[derive(Debug, thiserror::Error)]
pub enum WireError {
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("invalid message: {0}")]
    Invalid(String),
    #[error("truncated frame at byte {0}")]
    Truncated(usize),
    #[error("unsupported codec: {0}")]
    UnsupportedCodec(u8),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::control::{Caps, ControlMsg, Role};
    use crate::file::sanitize_rel_path;
    use crate::video::{encode_frame, FrameHeader, Rect};
    use base64::engine::general_purpose::STANDARD as B64;
    use base64::Engine;

    #[test]
    fn protocol_ids_registered() {
        // 字面量同步到 docs/protocol/registry.toml（scripts/check/protocol-registry.sh）
        assert_eq!(CONTROL_PROTOCOL_ID, "/rd/control/1");
        assert_eq!(VIDEO_PROTOCOL_ID, "/rd/video/1");
        assert_eq!(FILE_PROTOCOL_ID, "/rd/file/1");
    }

    #[test]
    fn control_hello_roundtrip() {
        let msg = ControlMsg::Hello {
            v: PROTOCOL_VERSION,
            role: Role::Viewer,
            session_id: "0123456789abcdef".into(),
            caps: Caps {
                audio: false,
                file: true,
                clipboard: true,
            },
        };
        let bytes = msg.encode().unwrap();
        assert_eq!(ControlMsg::decode(&bytes).unwrap(), msg);
    }

    #[test]
    fn input_message_roundtrip() {
        let msg = ControlMsg::InputMouse {
            x: 1234,
            y: 567,
            buttons: 0b101,
            wheel_dx: 0,
            wheel_dy: -3,
        };
        let bytes = msg.encode().unwrap();
        assert_eq!(ControlMsg::decode(&bytes).unwrap(), msg);
        let key = ControlMsg::InputKey {
            code: 0x0039,
            down: true,
            modifiers: 0x10,
        };
        assert_eq!(ControlMsg::decode(&key.encode().unwrap()).unwrap(), key);
    }

    #[test]
    fn hello_unknown_version_rejected() {
        let msg = ControlMsg::Hello {
            v: 99,
            role: Role::Host,
            session_id: "0123456789abcdef".into(),
            caps: Caps {
                audio: false,
                file: false,
                clipboard: false,
            },
        };
        let err = msg.encode().unwrap_err();
        assert!(err.to_string().contains("unsupported hello"));
    }

    #[test]
    fn bad_session_id_rejected() {
        let msg = ControlMsg::Hello {
            v: PROTOCOL_VERSION,
            role: Role::Host,
            session_id: "not-hex!".into(),
            caps: Caps {
                audio: false,
                file: false,
                clipboard: false,
            },
        };
        assert!(msg.encode().is_err());
    }

    #[test]
    fn unknown_type_rejected() {
        let bytes = br#"{"type":"nope","x":1}"#;
        assert!(ControlMsg::decode(bytes).is_err());
    }

    #[test]
    fn video_frame_roundtrip() {
        let header = FrameHeader {
            seq: 7,
            ts_ms: 1_700_000_000_123,
            w: 1280,
            h: 720,
            keyframe: true,
            codec: crate::video::CODEC_RAW_RGBA,
            rects: vec![Rect {
                x: 0,
                y: 0,
                w: 1280,
                h: 720,
            }],
        };
        let payload = vec![0xABu8; 8 * 1024];
        let bytes = encode_frame(&header, &payload).unwrap();
        let (got, got_payload) = decode_frame(&bytes).unwrap();
        assert_eq!(got, header);
        assert_eq!(got_payload, payload);
    }

    #[test]
    fn video_truncated_rejected() {
        let header = FrameHeader {
            seq: 1,
            ts_ms: 0,
            w: 10,
            h: 10,
            keyframe: false,
            codec: crate::video::CODEC_RAW_RGBA,
            rects: vec![],
        };
        let bytes = encode_frame(&header, &[1, 2, 3]).unwrap();
        assert!(decode_frame(&bytes[..bytes.len() - 2]).is_err());
    }

    #[test]
    fn video_bad_magic_and_codec_rejected() {
        let header = FrameHeader {
            seq: 1,
            ts_ms: 0,
            w: 10,
            h: 10,
            keyframe: false,
            codec: 0xEE,
            rects: vec![],
        };
        assert!(encode_frame(&header, &[]).is_err());
        let ok_header = FrameHeader {
            seq: 1,
            ts_ms: 0,
            w: 10,
            h: 10,
            keyframe: false,
            codec: crate::video::CODEC_RAW_RGBA,
            rects: vec![],
        };
        let bytes = encode_frame(&ok_header, &[]).unwrap();
        let mut tampered = bytes.clone();
        tampered[0] = 0x00; // 破坏 magic
        assert!(decode_frame(&tampered).is_err());
    }

    #[test]
    fn file_xfer_data_roundtrip() {
        let msg = crate::file::FileMsg::xfer_data("xfer-1".into(), 0, &[0u8, 1, 2, 255]).unwrap();
        let decoded =
            crate::file::FileMsg::decode(&crate::file::FileMsg::encode(&msg).unwrap()).unwrap();
        assert_eq!(decoded, msg);
        assert_eq!(decoded.xfer_data_raw().unwrap(), vec![0u8, 1, 2, 255]);
    }

    #[test]
    fn file_xfer_data_too_big_rejected() {
        let big = vec![0u8; MAX_DATA_RAW_BYTES + 1];
        assert!(crate::file::FileMsg::xfer_data("x".into(), 0, &big).is_err());
    }

    #[test]
    fn file_xfer_data_b64_roundtrip_with_b64_helper() {
        let msg = crate::file::FileMsg::xfer_data("id".into(), 10, b"payload").unwrap();
        if let FileMsg::XferData { data, .. } = &msg {
            assert_eq!(B64.decode(data).unwrap(), b"payload");
        } else {
            panic!("expected XferData");
        }
    }

    #[test]
    fn sanitize_path_rejects_escape() {
        assert!(sanitize_rel_path(".").is_err());
        assert!(sanitize_rel_path("/etc/passwd").is_err());
        assert!(sanitize_rel_path("a/../../b").is_err());
        assert!(sanitize_rel_path("a/b/..").is_err());
        assert!(sanitize_rel_path("").is_err());
        assert!(sanitize_rel_path("a\0b").is_err());
        assert_eq!(
            sanitize_rel_path("Downloads/x.txt").unwrap(),
            "Downloads/x.txt"
        );
        assert_eq!(sanitize_rel_path("a/b/c").unwrap(), "a/b/c");
        assert_eq!(sanitize_rel_path("文件/说明.md").unwrap(), "文件/说明.md");
    }
}

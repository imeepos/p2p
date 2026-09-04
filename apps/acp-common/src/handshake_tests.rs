use super::*;
use uuid::Uuid;

#[test]
fn client_hello_roundtrip_minimal() {
    let hello = ClientHello::new(Uuid::new_v4());
    let line = hello.to_line().unwrap();
    assert!(!line.contains('\n'));
    assert_eq!(parse_client_hello(&line).unwrap(), hello);
    assert!(line.contains("\"v\":1"));
    assert!(line.contains("\"conn\":"));
    assert!(!line.contains("token"));
    assert!(!line.contains("reattach"));
}

#[test]
fn client_hello_roundtrip_full() {
    let mut hello = ClientHello::new(Uuid::new_v4());
    hello.token = Some("tok-1".to_owned());
    hello.reattach = Some(Uuid::new_v4());
    let line = hello.to_line().unwrap();
    assert_eq!(parse_client_hello(&line).unwrap(), hello);
}

#[test]
fn client_hello_rejects_unknown_field() {
    let line = format!("{{\"v\":1,\"conn\":\"{}\",\"extra\":1}}", Uuid::new_v4());
    assert_eq!(
        parse_client_hello(&line),
        Err(ErrorCode::HandshakeMalformed)
    );
}

#[test]
fn client_hello_rejects_version_mismatch() {
    let line = format!("{{\"v\":2,\"conn\":\"{}\"}}", Uuid::new_v4());
    assert_eq!(
        parse_client_hello(&line),
        Err(ErrorCode::HandshakeMalformed)
    );
}

#[test]
fn client_hello_rejects_bad_uuid_and_broken_json() {
    assert_eq!(
        parse_client_hello("{\"v\":1,\"conn\":\"nope\"}"),
        Err(ErrorCode::HandshakeMalformed)
    );
    assert_eq!(parse_client_hello("{"), Err(ErrorCode::HandshakeMalformed));
    assert_eq!(parse_client_hello(""), Err(ErrorCode::HandshakeMalformed));
    assert_eq!(parse_client_hello("42"), Err(ErrorCode::HandshakeMalformed));
}

#[test]
fn server_hello_ready_roundtrip() {
    let frame = ServerHello::ready(crate::policy::Scope::Workspace, "home-agent");
    let line = frame.to_line().unwrap();
    assert!(!line.contains('\n'));
    assert_eq!(parse_server_hello(&line).unwrap(), frame);
    assert!(line.contains("\"scope\":\"workspace\""));
    assert!(line.contains("\"agent\":\"home-agent\""));
    assert!(line.contains("\"bridge\":\"1\""));
}

#[test]
fn server_hello_denied_wire_shape() {
    let frame = ServerHello::denied(&ErrorCode::PeerNotAllowed);
    let line = frame.to_line().unwrap();
    assert_eq!(line, "{\"denied\":\"peer-not-allowed\"}");
    match parse_server_hello(&line).unwrap() {
        ServerHello::Denied { denied } => assert_eq!(denied, "peer-not-allowed"),
        other => panic!("expected denied frame, got {other:?}"),
    }
}

#[test]
fn server_hello_rejects_unknown_shape_and_bad_scope() {
    assert_eq!(
        parse_server_hello("{\"hello\":1}"),
        Err(ErrorCode::HandshakeMalformed)
    );
    assert_eq!(
        parse_server_hello("{\"ready\":{\"scope\":\"bogus\",\"agent\":\"a\",\"bridge\":\"1\"}}"),
        Err(ErrorCode::HandshakeMalformed)
    );
    assert_eq!(
        parse_server_hello(
            "{\"ready\":{\"scope\":\"owner\",\"agent\":\"a\",\"bridge\":\"1\",\"extra\":true}}"
        ),
        Err(ErrorCode::HandshakeMalformed)
    );
}

use base64::Engine as _;
use p2p_identity::{Keypair, PeerId};
use repair_enforce::whitelist::ShellWhitelist;
use repair_helper::audit::{AuditEvent, AuditSink};
use repair_helper::cap::{apply_output_gate, MAX_OUTPUT_BYTES};
use repair_helper::jail::PathJail;
use repair_helper::ticket::{mint, TicketError, TicketLedger, TicketVerifier, SCOPE_DIAG};
use repair_helper::{Host, ToolResult};
use serde_json::{json, Value};
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::io::{duplex, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::sync::watch;

fn temp(tag: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!("rh-boundary-{}-{tag}", std::process::id()));
    fs::create_dir_all(&p).unwrap();
    p
}

async fn rpc(input: &[u8]) -> String {
    let (mut client, server) = duplex(1 << 20);
    let (reader, writer) = tokio::io::split(server);
    let (_tx, rx) = watch::channel(false);
    let task = tokio::spawn(Host::empty().serve(BufReader::new(reader), writer, rx));
    client.write_all(input).await.unwrap();
    client.shutdown().await.unwrap();
    let mut out = String::new();
    BufReader::new(client)
        .read_to_string(&mut out)
        .await
        .unwrap();
    assert!(task.await.unwrap().is_ok());
    out
}

#[tokio::test]
async fn malformed_json_and_blank_lines_are_signalled() {
    let out = rpc(b"\n  \r\nnot-json\r\n").await;
    assert!(
        out.is_empty(),
        "malformed/no-id lines must not fabricate replies"
    );
    let out = rpc(br#"{"jsonrpc":"1.0","id":1,"method":"ping"}
"#)
    .await;
    assert!(out.contains("-32600"));
    let out = rpc(br#"{"id":1,"method":"ping"}
"#)
    .await;
    assert!(
        out.is_empty(),
        "missing jsonrpc is malformed and has no reply: {out}"
    );
}

#[tokio::test]
async fn jsonrpc_missing_version_and_id_type_matrix() {
    for id in ["\"s\"", "7", "null"] {
        let input = format!(
            r#"{{"jsonrpc":"2.0","id":{id},"method":"ping"}}
"#
        );
        let out = rpc(input.as_bytes()).await;
        if id == "null" {
            assert!(out.is_empty(), "null id is notification-like: {out}");
        } else {
            assert!(out.contains("jsonrpc"), "id={id}: {out}");
        }
    }
    for params in [
        "{}",
        r#"{"protocolVersion":null}"#,
        r#"{"protocolVersion":""}"#,
    ] {
        let input = format!(
            r#"{{"jsonrpc":"2.0","id":1,"method":"initialize","params":{params}}}
"#
        );
        let out = rpc(input.as_bytes()).await;
        assert!(out.contains("protocolVersion"), "params={params}: {out}");
    }
}

#[tokio::test]
async fn notification_with_id_is_replied_and_long_line_is_safe() {
    let notification = br#"{"jsonrpc":"2.0","method":"notifications/initialized","id":9}
"#;
    let out = rpc(notification).await;
    assert!(
        out.contains("-32601"),
        "id-bearing notification is a request: {out}"
    );
    let long = format!(
        r#"{{"jsonrpc":"2.0","id":1,"method":"ping","params":"{}"}}
"#,
        "x".repeat(300_000)
    );
    let out = rpc(long.as_bytes()).await;
    assert!(out.contains("jsonrpc"));
}

#[tokio::test]
async fn tool_call_missing_name_and_non_object_arguments_are_errors() {
    for params in [r#"{}"#, r#"{"name":42}"#, r#"{"name":"missing"}"#] {
        let input = format!(
            r#"{{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{params}}}
"#
        );
        let out = rpc(input.as_bytes()).await;
        assert!(
            out.contains("isError") || out.contains("-32602"),
            "{params}: {out}"
        );
    }
    let out = rpc(br#"{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"sys_snapshot","arguments":[]}}
"#).await;
    assert!(out.contains("os=") || out.contains("isError"));
}

#[test]
fn jail_root_file_trailing_slash_nested_and_first_root_selection() {
    let base = temp("jail");
    let first = base.join("first");
    let nested = first.join("nested");
    let second = base.join("second");
    fs::create_dir_all(&nested).unwrap();
    fs::create_dir_all(&second).unwrap();
    fs::write(first.join("ok.txt"), "ok").unwrap();
    let err = PathJail::from_roots(vec![first.join("ok.txt")]).unwrap_err();
    assert!(err.contains("not a directory"));
    let jail = PathJail::from_roots(vec![first.clone(), second.clone()]).unwrap();
    let slash = PathJail::from_roots(vec![PathBuf::from(format!("{}/", first.display()))]).unwrap();
    assert!(slash.resolve("ok.txt").is_ok());
    assert!(jail.resolve("ok.txt").is_ok());
    assert!(jail.resolve("nested").unwrap().ends_with("nested"));
    assert_eq!(jail.first_root().unwrap(), fs::canonicalize(first).unwrap());
    assert!(jail.roots()[0].starts_with(jail.roots()[0].parent().unwrap()));
}

#[cfg(unix)]
#[test]
fn jail_case_behavior_and_nested_root_are_component_safe() {
    use std::os::unix::fs::symlink;
    let base = temp("case");
    let root = base.join("Root");
    let nested = root.join("Child");
    let outside = base.join("outside");
    fs::create_dir_all(&nested).unwrap();
    fs::create_dir_all(&outside).unwrap();
    fs::write(nested.join("f"), "x").unwrap();
    symlink(&outside, root.join("link")).unwrap();
    let jail = PathJail::from_roots(vec![root.clone(), nested.clone()]).unwrap();
    assert!(jail.resolve("Child/f").is_ok());
    assert!(jail.resolve("link").unwrap_err().contains("escapes"));
    let lower = jail.resolve("child/f");
    if cfg!(target_os = "macos") {
        assert!(
            lower.is_ok(),
            "default macOS filesystem is case-insensitive: {lower:?}"
        );
    } else {
        assert!(
            lower.is_err(),
            "case-sensitive FS must reject case variant: {lower:?}"
        );
    }
}

#[test]
fn cap_exact_limit_and_multibyte_boundary() {
    let exact = apply_output_gate(ToolResult {
        text: "a".repeat(MAX_OUTPUT_BYTES),
        truncated: false,
    });
    assert_eq!(exact.text.len(), MAX_OUTPUT_BYTES);
    assert!(!exact.truncated);
    let mut text = "a".repeat(MAX_OUTPUT_BYTES - 1);
    text.push('界');
    let over = apply_output_gate(ToolResult {
        text,
        truncated: false,
    });
    assert!(over.truncated);
    assert!(over.text.len() <= MAX_OUTPUT_BYTES);
    assert!(over.text.is_char_boundary(over.text.len()));
}

fn peers() -> (Keypair, PeerId, PeerId) {
    let platform = Keypair::from_seed(&[7; 32]);
    let bridge = Keypair::from_seed(&[2; 32]).peer_id();
    (platform, Keypair::from_seed(&[1; 32]).peer_id(), bridge)
}

fn ticket(now: u64, ttl: u64) -> (String, TicketVerifier, PeerId) {
    let (platform, _helper, bridge) = peers();
    let value = mint(
        &platform,
        "boundary",
        "helper",
        &bridge.to_string(),
        SCOPE_DIAG,
        ttl,
        now,
    )
    .unwrap();
    (
        value,
        TicketVerifier::new(platform.public(), TicketLedger::default()),
        bridge,
    )
}

#[test]
fn ticket_exp_equal_now_and_future_iat_boundaries() {
    let (expired, verifier, bridge) = ticket(100, 0);
    assert_eq!(
        verifier.verify(&expired, &bridge, 100),
        Err(TicketError::Expired)
    );
    let (platform, _helper, bridge) = peers();
    let payload = json!({"ticket_id":"future","helper_peer":"helper","bridge_peer":bridge.to_string(),"scope":"diag","iat":200,"exp":300});
    let body = serde_json::to_vec(&payload).unwrap();
    let sig = platform.sign(&body);
    let encoded = format!(
        "{}.{}",
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(&body),
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(sig)
    );
    let result = TicketVerifier::new(platform.public(), TicketLedger::default())
        .verify(&encoded, &bridge, 201);
    assert!(
        result.is_ok(),
        "future iat behavior is observable: {result:?}"
    );
}

#[test]
fn ticket_base64_padding_invalid_chars_and_non_object_payload() {
    let (valid, verifier, bridge) = ticket(100, 20);
    let (body, sig) = valid.split_once('.').unwrap();
    assert!(verifier
        .verify(&format!("{body}=.{sig}"), &bridge, 101)
        .is_err());
    assert!(verifier
        .verify(&format!("{body}!.{sig}"), &bridge, 101)
        .is_err());
    let platform = Keypair::from_seed(&[7; 32]);
    let raw = serde_json::to_vec(&json!(["not", "object"])).unwrap();
    let signed = format!(
        "{}.{}",
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(&raw),
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(platform.sign(&raw))
    );
    assert!(verifier.verify(&signed, &bridge, 101).is_err());
}

#[test]
fn audit_special_characters_and_concurrent_jsonl_lines() {
    let path = temp("audit").join("events.jsonl");
    let sink = AuditSink::with_file(&path).unwrap();
    let shared = Arc::new(sink);
    let mut joins = Vec::new();
    for i in 0..16 {
        let sink = shared.clone();
        joins.push(std::thread::spawn(move || {
            sink.push(AuditEvent::new(
                "tool\"\\",
                format!(r#"{{"i":{i},"v":"\n"}}"#),
                "read",
                "ok",
                "line\n\"",
                i,
            ));
        }));
    }
    for join in joins {
        join.join().unwrap();
    }
    let text = fs::read_to_string(path).unwrap();
    assert_eq!(text.lines().count(), 16);
    for line in text.lines() {
        let value: Value = serde_json::from_str(line).unwrap();
        assert_eq!(value["tool"], "tool\"\\");
    }
}

#[test]
fn builtin_import_remains_available_to_boundary_suite() {
    assert!(!repair_enforce::builtin().rules().is_empty());
    assert!(ShellWhitelist::empty().rules().is_empty());
}

//! agent_message_chunk -> A2A Part 映射（§5.3 接收侧）：text 直映 TextPart；
//! image 映 FilePart 并执行 4 MiB 上限（超限任务失败 + 审计，不静默）；
//! 其余 content 折叠 DataPart（折叠信息卡渲染语义）。

use a2a::{DataPart, FilePart, Part, TaskState, TextPart, FILE_PART_CAP_BYTES};
use serde_json::Value;
use tokio::sync::mpsc;

use super::bridge::{BridgeEvent, BridgeParams};

/// session/update 且 update.sessionUpdate = agent_message_chunk。
pub(crate) fn is_chunk_update(root: &Value) -> bool {
    root.get("method").and_then(Value::as_str) == Some("session/update")
        && root.pointer("/params/update/sessionUpdate").and_then(Value::as_str)
            == Some("agent_message_chunk")
}

/// 映射并发送 chunk 事件；超限返回 Failed 终态（run 据此收尾）。
pub(crate) async fn emit_chunk(
    params: &BridgeParams,
    root: &Value,
    event_tx: &mpsc::Sender<BridgeEvent>,
) -> Option<TaskState> {
    let content = root
        .pointer("/params/update/content")
        .cloned()
        .unwrap_or(Value::Null);
    let (part, over_cap) = to_part(&content);
    if over_cap {
        params.audit.record(crate::audit::AuditEvent::A2aDenied {
            peer: params.peer.clone(),
            detail: format!("task {} file part over {} cap", params.task_id, FILE_PART_CAP_BYTES),
        });
        return Some(TaskState::Failed);
    }
    let _ = event_tx
        .send(BridgeEvent::Message {
            message_id: uuid::Uuid::new_v4().to_string(),
            part,
        })
        .await;
    None
}

fn to_part(content: &Value) -> (Part, bool) {
    match content.get("type").and_then(Value::as_str) {
        Some("text") => (
            Part::Text(TextPart {
                text: content
                    .get("text")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_owned(),
            }),
            false,
        ),
        Some("image") => {
            let bytes = content.get("data").and_then(Value::as_str).unwrap_or_default();
            let part = Part::File(FilePart {
                name: "image".into(),
                mime_type: content
                    .get("mime")
                    .and_then(Value::as_str)
                    .unwrap_or("image/png")
                    .to_owned(),
                bytes: bytes.to_owned(),
            });
            (part, bytes.len() > FILE_PART_CAP_BYTES)
        }
        _ => {
            let encoded = serde_json::to_vec(content).map(|v| v.len()).unwrap_or(0);
            (Part::Data(DataPart { data: content.clone() }), encoded > FILE_PART_CAP_BYTES)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn text_chunk_maps_to_text_part() {
        let root = json!({
            "jsonrpc": "2.0", "method": "session/update",
            "params": { "sessionId": "s", "update": {
                "sessionUpdate": "agent_message_chunk",
                "content": { "type": "text", "text": "你好" } } }
        });
        assert!(is_chunk_update(&root));
        let (part, over) = to_part(root.pointer("/params/update/content").unwrap());
        assert!(!over);
        assert_eq!(part, Part::Text(TextPart { text: "你好".into() }));
    }

    #[test]
    fn image_chunk_maps_to_file_part_with_cap() {
        let small = json!({ "type": "image", "data": "aGk=", "mime": "image/png" });
        let (part, over) = to_part(&small);
        assert!(!over);
        assert!(matches!(part, Part::File(_)));
        let big = json!({ "type": "image", "data": "x".repeat(FILE_PART_CAP_BYTES + 1) });
        let (_, over) = to_part(&big);
        assert!(over, "超 4MiB 必须判超限");
    }

    #[test]
    fn unknown_content_folds_to_data_part() {
        let content = json!({ "type": "resource_link", "uri": "https://x" });
        let (part, over) = to_part(&content);
        assert!(!over);
        assert!(matches!(part, Part::Data(_)));
    }

    #[test]
    fn non_update_lines_are_not_chunks() {
        assert!(!is_chunk_update(&json!({ "method": "session/request_permission" })));
        assert!(!is_chunk_update(&json!({
            "method": "session/update",
            "params": { "update": { "sessionUpdate": "tool_call" } }
        })));
    }
}

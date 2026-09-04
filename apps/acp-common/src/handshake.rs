//! 握手帧编解码：桥上唯一协议插入点，每连接恰一行（设计 §4.1/§4.2-2）。
//! 未知字段策略：拒绝（deny_unknown_fields）——握手行是信任边界，容忍未知字段
//! 会把协议漂移推迟成运行期错位，此处 fail-fast 并入审计日志。

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::consts::{BRIDGE_VERSION, HANDSHAKE_VERSION};
use crate::error::ErrorCode;
use crate::policy::Scope;

/// 客户端→桥：单行 JSON {"v":1,"conn":"<uuid>","token?":"...","reattach?":"<uuid>"}。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClientHello {
    pub v: u32,
    pub conn: Uuid,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reattach: Option<Uuid>,
}

/// ready 载荷：scope 通告 + agent 名 + 桥协议版本。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Ready {
    pub scope: Scope,
    pub agent: String,
    pub bridge: String,
}

/// 桥→客户端：{"ready":{...}} 或 {"denied":"<错误码>"}（untagged 二选一）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ServerHello {
    Ready { ready: Ready },
    Denied { denied: String },
}

impl ClientHello {
    pub fn new(conn: Uuid) -> Self {
        Self {
            v: HANDSHAKE_VERSION,
            conn,
            token: None,
            reattach: None,
        }
    }

    /// 编码为单行 JSON（ndjson 行界由传输层保证）。
    pub fn to_line(&self) -> Result<String, ErrorCode> {
        encode_line(self)
    }
}

impl Ready {
    pub fn new(scope: Scope, agent: &str) -> Self {
        Self {
            scope,
            agent: agent.to_owned(),
            bridge: BRIDGE_VERSION.to_owned(),
        }
    }
}

impl ServerHello {
    pub fn ready(scope: Scope, agent: &str) -> Self {
        Self::Ready {
            ready: Ready::new(scope, agent),
        }
    }

    pub fn denied(code: &ErrorCode) -> Self {
        Self::Denied {
            denied: code.code().to_owned(),
        }
    }

    pub fn to_line(&self) -> Result<String, ErrorCode> {
        encode_line(self)
    }
}

/// 解析客户端握手行：非法 JSON/字段/uuid 或 v 不符 → HandshakeMalformed。
pub fn parse_client_hello(line: &str) -> Result<ClientHello, ErrorCode> {
    let hello: ClientHello = serde_json::from_str(line).map_err(malformed)?;
    if hello.v != HANDSHAKE_VERSION {
        eprintln!("acp-common: handshake version mismatch: v={}", hello.v);
        return Err(ErrorCode::HandshakeMalformed);
    }
    Ok(hello)
}

/// 解析桥握手回执行。
pub fn parse_server_hello(line: &str) -> Result<ServerHello, ErrorCode> {
    serde_json::from_str(line).map_err(malformed)
}

/// 序列化失败对本组静态类型不可达；仍显式上抛并留日志信号，绝不 unwrap。
fn encode_line<T: Serialize>(frame: &T) -> Result<String, ErrorCode> {
    serde_json::to_string(frame).map_err(|err| {
        eprintln!("acp-common: handshake encode failed: {err}");
        ErrorCode::HandshakeMalformed
    })
}

fn malformed(err: serde_json::Error) -> ErrorCode {
    eprintln!("acp-common: handshake malformed: {err}");
    ErrorCode::HandshakeMalformed
}

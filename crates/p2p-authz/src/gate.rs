//! chat PEP 判定核心（authz-role-design Amended A-1）：chat.send / chat.attachment
//! 两闸判定 + `authz.denied` 审计（含 peer/perm/reason）+ 读失败=拒（红线 2）。
//! CheckGate trait 壳在消费面装配处（§3：authz 零依赖业务 crate、chat 不感知 authz）。
//! 对端无从区分「未绑/过期/掉线」（不回 wire），拒绝细节只进本机审计。

use std::path::Path;

use crate::decision::Decision;
use crate::permissions::Permission;
use crate::{audit::AuditEvent, Authz, SystemClock};

/// 入站判定拒绝：reason 为 reason 码（NotBound/Expired/BrokenRole/MissingPerm）
/// 或 ReadFailed；消费面只断流记日志，不回 wire。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatGateDeny {
    pub reason: String,
}

/// 入站单聊准入：media=false 判 chat.send；media=true 判 chat.attachment
/// （先 send 后 attachment 的次序由 chat 侧编排）。Deny 四因与存储读失败
/// 一律 Err 并落 `authz.denied` 审计；Allow 静默放行（不落审计）。
pub fn chat_admit(data_dir: &Path, peer: &str, media: bool) -> Result<(), ChatGateDeny> {
    let perm = if media {
        Permission::CHAT_ATTACHMENT
    } else {
        Permission::CHAT_SEND
    };
    let outcome = Authz::new(data_dir, SystemClock).check(peer, perm);
    let (code, detail) = match outcome {
        Ok(Decision::Allow) => return Ok(()),
        Ok(Decision::Deny(reason)) => (reason.code(), String::new()),
        Err(e) => ("ReadFailed", format!(": {e}")),
    };
    crate::audit::record(
        data_dir,
        &AuditEvent::denied(format!(
            "chat peer={peer} perm={perm} reason={code}{detail}"
        )),
    );
    Err(ChatGateDeny {
        reason: code.to_owned(),
    })
}

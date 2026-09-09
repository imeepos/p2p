//! 审计 sink（authz-a3-plan §1 S2 P1b）：<data>/authz/audit.jsonl append-only，
//! 一行一个事件。事件形态对齐社区惯例：`{at(RFC3339), actor:"owner", kind,
//! before, after, note}`；kind 闭集五类（授权变更与判定拒绝），before/after
//! 为变更前后快照（无则 null）。写失败 `tracing::error` 且不阻塞主操作
//! （失败必有日志信号，禁止静默丢）；不做哈希链/轮转（plan §4 克制条款）。

use std::io::Write;
use std::path::{Path, PathBuf};

use serde::Serialize;
use serde_json::{json, Value};

use crate::binding::Binding;
use crate::clock::{Clock, SystemClock};
use crate::ops_binding::BoundBinding;
use crate::role::Role;

/// 审计文件名（与 roles.json/bindings.json 同在 authz/ 子目录）。
pub const AUDIT_FILE: &str = "audit.jsonl";

/// 审计事件 kind 闭集（authz-a3-plan §1 S2 P1b）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum AuditKind {
    /// 自定义角色创建。
    #[serde(rename = "authz.role.created")]
    RoleCreated,
    /// peer 绑定角色（新建或 upsert）。
    #[serde(rename = "authz.bound")]
    Bound,
    /// peer 解绑。
    #[serde(rename = "authz.unbound")]
    Unbound,
    /// 自定义角色删除。
    #[serde(rename = "authz.role.deleted")]
    RoleDeleted,
    /// 判定拒绝（各 PEP 按面采样，acp 面复用既有 AuthzDenied 不双写）。
    #[serde(rename = "authz.denied")]
    Denied,
}

impl AuditKind {
    pub fn as_str(self) -> &'static str {
        match self {
            AuditKind::RoleCreated => "authz.role.created",
            AuditKind::Bound => "authz.bound",
            AuditKind::Unbound => "authz.unbound",
            AuditKind::RoleDeleted => "authz.role.deleted",
            AuditKind::Denied => "authz.denied",
        }
    }
}

/// 审计事件：before/after 为快照 Value（无则 null），at 为 RFC3339 UTC。
#[derive(Debug, Clone, Serialize)]
pub struct AuditEvent {
    pub at: String,
    pub actor: &'static str,
    pub kind: AuditKind,
    pub before: Value,
    pub after: Value,
    pub note: String,
}

impl AuditEvent {
    fn new(kind: AuditKind, before: Value, after: Value, note: impl Into<String>) -> Self {
        Self {
            at: rfc3339_utc(SystemClock.now_unix()),
            actor: "owner",
            kind,
            before,
            after,
            note: note.into(),
        }
    }

    /// 角色创建：before=null，after=角色快照。
    pub fn role_created(role: &Role) -> Self {
        Self::new(AuditKind::RoleCreated, Value::Null, role_snapshot(role), "")
    }

    /// 角色删除：before=角色快照，after=null。
    pub fn role_deleted(role: &Role) -> Self {
        Self::new(AuditKind::RoleDeleted, role_snapshot(role), Value::Null, "")
    }

    /// 绑定：before=null，after=绑定快照；upsert 更新以 note 区分
    /// （p2p-authz 管理面 API 不回吐旧绑定，克制不为此扩 API）。
    pub fn bound(bound: &BoundBinding) -> Self {
        let note = if bound.created { "" } else { "upsert" };
        Self::new(
            AuditKind::Bound,
            Value::Null,
            binding_snapshot(&bound.binding),
            note,
        )
    }

    /// 解绑：before=绑定快照，after=null。
    pub fn unbound(binding: &Binding) -> Self {
        Self::new(AuditKind::Unbound, binding_snapshot(binding), Value::Null, "")
    }

    /// 判定拒绝：无状态变更，note 携带面/对象/reason 码（不泄细节，§7）。
    pub fn denied(note: impl Into<String>) -> Self {
        Self::new(AuditKind::Denied, Value::Null, Value::Null, note)
    }
}

/// 角色快照（camelCase，与 CLI 报告同口径）。
pub fn role_snapshot(role: &Role) -> Value {
    json!({
        "roleId": role.role_id,
        "name": role.name,
        "permissions": role.permissions.iter().map(|p| p.as_str()).collect::<Vec<_>>(),
        "builtin": role.builtin,
        "note": role.note,
    })
}

/// 绑定快照。
pub fn binding_snapshot(binding: &Binding) -> Value {
    json!({
        "peerId": binding.peer_id,
        "roleId": binding.role_id,
        "grantedAt": binding.granted_at,
        "expiresAt": binding.expires_at,
        "note": binding.note,
    })
}

/// 审计文件路径：<data>/authz/audit.jsonl。
pub fn audit_path(data_dir: &Path) -> PathBuf {
    data_dir.join(crate::store::AUTHZ_DIR).join(AUDIT_FILE)
}

/// 落账一条事件（append-only）。失败仅留 error 日志，不阻塞主操作、不上抛
/// （审计是旁路，主操作成功不因审计失败回滚）；禁止静默丢。
pub fn record(data_dir: &Path, event: &AuditEvent) {
    if let Err(e) = append_jsonl(data_dir, event) {
        tracing::error!(
            error = %e,
            kind = event.kind.as_str(),
            path = %audit_path(data_dir).display(),
            "authz 审计写失败，事件未落盘"
        );
    }
}

/// 落账可测内核：目录缺失自建、append 打开、单行写入；错误上抛由 [record] 转日志。
pub fn append_jsonl(data_dir: &Path, event: &AuditEvent) -> std::io::Result<()> {
    let path = audit_path(data_dir);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let line = serde_json::to_string(event)
        .map_err(|e| std::io::Error::other(format!("audit serialize: {e}")))?;
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)?;
    writeln!(file, "{line}")
}

/// Unix 秒 → RFC3339 UTC（秒级，如 2026-02-29T00:00:00Z）。
/// 算法沿 p2p-cli/llm_share 同款（Howard Hinnant civil_from_days）；
/// 依赖方向（authz 零依赖业务 crate）不允许直接复用，故同算法落位。
pub fn rfc3339_utc(secs: u64) -> String {
    let days = (secs / 86_400) as i64;
    let rem = secs % 86_400;
    let (year, month, day) = civil_from_days(days);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        rem / 3_600,
        (rem % 3_600) / 60,
        rem % 60
    )
}

/// Howard Hinnant civil_from_days：Unix 天数 → (年, 月, 日)。
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let year = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    (if month <= 2 { year + 1 } else { year }, month as u32, day)
}

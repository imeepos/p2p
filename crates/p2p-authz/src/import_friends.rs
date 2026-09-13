//! 存量好友回填（Amended A-4）：扫描好友簿全集，「有好友关系 && 无 authz 绑定」
//! 绑 default_role；已有绑定（含已过期——过期是判定语义，不改写用户显式绑定）、
//! 未配置 default_role、default_role 角色不存在的均跳过；只增不删（不解绑、不改
//! 既有绑定的角色与过期时间）；重跑幂等（二次零变更）；每次执行落
//! `authz.import.friends` 审计（含 bound/skipped 数）。入口：p2pctl authz import
//! friends 子命令 + GUI/daemon 节点启动装配自动执行一次（升级后老好友聊天零中断）。

use std::path::Path;

use serde::Serialize;

use crate::audit::AuditEvent;
use crate::{Authz, AuthzError, SystemClock};

/// 回填报告：scanned=入参好友数；bound=本次新建；skipped=已有绑定或角色缺失；
/// disabled=未配置 default_role（空串），未触存储与审计。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportFriendsReport {
    pub scanned: usize,
    pub bound: usize,
    pub skipped: usize,
    pub disabled: bool,
}

/// 幂等回填：peer 顺序以入参为准（调用方从好友簿读出）。
/// 读失败（表损坏/不可写）显式报错上抛（红线 2），由调用方决定命令失败或降级告警。
pub fn import_friends(
    data_dir: &Path,
    friend_peers: &[String],
    default_role: &str,
) -> Result<ImportFriendsReport, AuthzError> {
    if default_role.is_empty() {
        return Ok(ImportFriendsReport {
            scanned: friend_peers.len(),
            bound: 0,
            skipped: friend_peers.len(),
            disabled: true,
        });
    }
    let authz = Authz::new(data_dir, SystemClock);
    if let Err(e) = authz.show_role(default_role) {
        // 角色不存在：整体跳过（不部分绑定），执行事实仍落审计；
        // 存储读失败等其他错误显式上抛（红线 2，禁止静默半跑）。
        return match e {
            AuthzError::RoleNotFound(_) => {
                audit_summary(data_dir, 0, friend_peers.len());
                Ok(ImportFriendsReport {
                    scanned: friend_peers.len(),
                    bound: 0,
                    skipped: friend_peers.len(),
                    disabled: false,
                })
            }
            other => Err(other),
        };
    }
    let mut bound = 0;
    let mut skipped = 0;
    for peer in friend_peers {
        // 每次重读磁盘（跨进程新鲜度，§6）；已有绑定（含过期）跳过，只增不删。
        if authz.load_engine()?.binding(peer).is_some() {
            skipped += 1;
            continue;
        }
        let event = authz.bind(peer, default_role, None, "import friends")?;
        crate::audit::record(data_dir, &AuditEvent::bound(&event));
        bound += 1;
    }
    audit_summary(data_dir, bound, skipped);
    Ok(ImportFriendsReport {
        scanned: friend_peers.len(),
        bound,
        skipped,
        disabled: false,
    })
}

/// 执行摘要落账：after 携带 bound/skipped 结构化计数（A-4）。
fn audit_summary(data_dir: &Path, bound: usize, skipped: usize) {
    crate::audit::record(data_dir, &AuditEvent::import_friends(bound, skipped));
}

//! 对端地址自学习（F1 配套）：入站 chat/invite 帧携带发端声明地址，回写好友簿
//! addrs（新地址优先、去重、有界），修复对端端口/地址变更后好友簿旧地址失联。
//! 学习边界：仅限已在簿好友（陌生人帧不回写）；声明地址由发端取 serve 发布的
//! advertised（无则不携带，禁止一次性进程的即弃监听地址污染好友簿）。

use crate::core::ChatCore;
use crate::model::parse_peer_id;

/// 好友簿单条 addrs 上限：合并后截断（有界）。
pub(crate) const MAX_FRIEND_ADDRS: usize = 4;

/// 合并地址：incoming 优先（新近可达性），existing 兜底，去重后截断。
fn merge_addrs(existing: &[String], incoming: &[String]) -> Vec<String> {
    let mut merged: Vec<String> = Vec::with_capacity(MAX_FRIEND_ADDRS);
    for addr in incoming.iter().chain(existing.iter()) {
        if merged.len() >= MAX_FRIEND_ADDRS {
            break;
        }
        if !merged.contains(addr) {
            merged.push(addr.clone());
        }
    }
    merged
}

/// 入站帧地址回写好友簿：非好友静默跳过；节点簿登记/落盘失败留 warn 不阻断收信。
pub(crate) fn learn_friend_addrs(core: &ChatCore, peer_id: &str, incoming: &[String]) {
    if incoming.is_empty() {
        return;
    }
    let Ok(peer) = parse_peer_id(peer_id) else {
        return;
    };
    let mut friends = match core.store.friends_list() {
        Ok(f) => f,
        Err(e) => {
            tracing::warn!(peer = %peer_id, error = %e, "地址自学习读好友簿失败");
            return;
        }
    };
    let Some(slot) = friends.iter_mut().find(|f| f.peer_id == peer_id) else {
        return;
    };
    let mut verified: Vec<&String> = Vec::new();
    for addr in incoming {
        match core.node.add_peer_address(peer, addr) {
            Ok(()) => verified.push(addr),
            Err(e) => {
                tracing::warn!(peer = %peer_id, addr = %addr, error = %e, "学习地址非法，剔除");
            }
        }
    }
    let verified_refs: Vec<String> = verified.into_iter().cloned().collect();
    let merged = merge_addrs(&slot.addrs, &verified_refs);
    if merged == slot.addrs {
        return;
    }
    slot.addrs = merged;
    if let Err(e) = core.store.upsert_friend(slot.clone()) {
        tracing::warn!(peer = %peer_id, error = %e, "学习地址回写好友簿失败");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn a(s: &str) -> String {
        s.to_string()
    }

    #[test]
    fn merge_puts_incoming_first_dedups_and_bounds() {
        let existing = vec![a("ip/u1"), a("ip/u2")];
        let incoming = vec![a("ip/u2"), a("ip/u3")];
        let merged = merge_addrs(&existing, &incoming);
        assert_eq!(
            merged,
            vec![a("ip/u2"), a("ip/u3"), a("ip/u1")],
            "新地址优先，旧地址兜底，去重保序"
        );
        let many: Vec<String> = (0..10).map(|i| a(&format!("ip/u{i}"))).collect();
        assert_eq!(merge_addrs(&[], &many).len(), MAX_FRIEND_ADDRS, "有界截断");
    }

    #[test]
    fn merge_no_change_when_incoming_is_existing_prefix() {
        let existing = vec![a("ip/u1"), a("ip/u2")];
        let merged = merge_addrs(&existing, &[a("ip/u1")]);
        assert_eq!(merged, existing, "无新地址不产生写盘");
    }
}

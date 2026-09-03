//! 选路辅助（纯函数）：按模型过滤 + spare 降序 + retention 透出，供 proxy 后续消费。

use crate::offer::SignedOffer;

/// 一条选路候选。retention 原样透出（§7.3 如实告知），语义解释交使用方裁量。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RouteCandidate {
    pub peer: String,
    /// 声明闲量（token），降序主键。
    pub spare: u64,
    /// 单请求上限；None = 未显式设限（预授权仍实校）。
    pub max_per_req: Option<u64>,
    pub retention: String,
}

/// 从声明集中选出支持 model 的候选：TTL 过期或该模型闲量为零者剔除，
/// 按 spare 降序、同 spare 按 peer 字典序（确定性），不截断数量——
/// 重试与熔断由 proxy 决策，这里只提供有序视图。
pub fn select_offers<'a, I>(offers: I, model: &str, now: u64) -> Vec<RouteCandidate>
where
    I: IntoIterator<Item = &'a SignedOffer>,
{
    let mut out: Vec<RouteCandidate> = offers
        .into_iter()
        .filter(|signed| now < signed.expires_at())
        .filter_map(|signed| {
            let spare = *signed.offer.spare.get(model)?;
            if spare == 0 {
                return None;
            }
            Some(RouteCandidate {
                peer: signed.offer.peer.clone(),
                spare,
                max_per_req: signed.offer.max_per_req.get(model).copied(),
                retention: signed.offer.retention.clone(),
            })
        })
        .collect();
    out.sort_by(|a, b| b.spare.cmp(&a.spare).then_with(|| a.peer.cmp(&b.peer)));
    out
}

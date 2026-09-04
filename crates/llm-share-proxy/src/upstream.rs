//! 上游访问抽象（§6）：出借方 key 只在这一层注入；真实 HTTPS 实现见 [crate::upstream_http]，
//! 测试用进程内 mock（禁外网）。SSE 以字节分片按到达顺序透出。

use futures::stream::BoxStream;
use serde_json::Value;

/// 一次上游 chat completions 调用。
pub struct UpstreamCall {
    pub base_url: String,
    pub model: String,
    pub api_key: String,
    pub body: Value,
}

/// 上游 SSE 字节流；实现方保证按到达顺序产出、结束时自然收束或显式报错。
pub type SseByteStream = BoxStream<'static, Result<Vec<u8>, UpstreamFailure>>;

/// 上游失败：HTTP 状态码可区分（429/5xx 透传），流中断单列（断流计费入口）。
#[derive(Debug, thiserror::Error)]
pub enum UpstreamFailure {
    #[error("upstream rejected: status {0}")]
    Rejected(u16),
    #[error("upstream stream broken: {0}")]
    Broken(String),
}

#[async_trait::async_trait]
pub trait Upstream: Send + Sync {
    /// 连接/HTTP 层失败以返回值呈现；流中途失败以流内错误元素呈现。
    async fn chat(&self, call: UpstreamCall) -> Result<SseByteStream, UpstreamFailure>;
}

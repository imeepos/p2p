//! reqwest(rustls) 上游客户端：POST {base}/chat/completions，注入出借方 key，
//! SSE 字节流透传。仅生产装配使用；测试路径走进程内 mock，不出网。

use futures::StreamExt;

use crate::upstream::{SseByteStream, Upstream, UpstreamCall, UpstreamFailure};

pub struct HttpUpstream {
    client: reqwest::Client,
}

impl HttpUpstream {
    /// 连接超时 10s：上游不可达尽早显式失败，不占出借方并发额度。
    pub fn new() -> std::io::Result<Self> {
        let client = reqwest::Client::builder()
            .user_agent(concat!("llm-share-proxy/", env!("CARGO_PKG_VERSION")))
            .connect_timeout(std::time::Duration::from_secs(10))
            .build()
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e.to_string()))?;
        Ok(Self { client })
    }
}

#[async_trait::async_trait]
impl Upstream for HttpUpstream {
    async fn chat(&self, call: UpstreamCall) -> Result<SseByteStream, UpstreamFailure> {
        let bytes = serde_json::to_vec(&call.body)
            .map_err(|e| UpstreamFailure::Broken(format!("encode body: {e}")))?;
        let url = format!("{}/chat/completions", call.base_url.trim_end_matches('/'));
        let resp = self
            .client
            .post(url)
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .bearer_auth(&call.api_key)
            .body(bytes)
            .send()
            .await
            .map_err(|e| UpstreamFailure::Broken(e.to_string()))?;
        let status = resp.status();
        if !status.is_success() {
            return Err(UpstreamFailure::Rejected(status.as_u16()));
        }
        let sse = resp.bytes_stream().map(|chunk| {
            chunk
                .map(|b| b.to_vec())
                .map_err(|e| UpstreamFailure::Broken(e.to_string()))
        });
        Ok(Box::pin(sse))
    }
}

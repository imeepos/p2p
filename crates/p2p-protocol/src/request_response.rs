//! request-response 原语（design §4/§11.3）。
//!
//! 流程：开流 → 写协议 ID → 写请求帧 → 读一帧回应 → 关流。
//! 一个 timeout 覆盖全程，任何环节卡住都返回 [ProtocolError::Timeout]，
//! 不 panic、不悬挂；本地帧超限映射为 FrameTooLarge，错误可区分。

use std::time::Duration;

use p2p_identity::PeerId;
use tokio::time;

use crate::{
    flatten_io, open_with_protocol, read_frame, write_frame, ProtocolError, ProtocolId,
    RequestResponse, StreamFactory,
};

/// [RequestResponse] 的默认实现：每次请求开一条新流，一问一答后即关流。
pub struct RequestResponseClient<F: StreamFactory> {
    factory: F,
}

impl<F: StreamFactory> RequestResponseClient<F> {
    pub fn new(factory: F) -> Self {
        Self { factory }
    }
}

#[async_trait::async_trait]
impl<F: StreamFactory> RequestResponse for RequestResponseClient<F> {
    async fn request(
        &self,
        peer: PeerId,
        id: ProtocolId,
        payload: Vec<u8>,
        timeout: Duration,
    ) -> Result<Vec<u8>, ProtocolError> {
        let call = async {
            let opened = self.factory.open_stream(&peer, &id).await?;
            let mut stream = open_with_protocol(opened, &id).await?;
            write_frame(&mut stream, &payload).await?;
            read_frame(&mut stream).await
        };
        match time::timeout(timeout, call).await {
            Ok(res) => res.map_err(flatten_io),
            Err(_) => Err(ProtocolError::Timeout(timeout)),
        }
    }
}

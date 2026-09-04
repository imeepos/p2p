//! /dsh-acp/1 handler：把 facade 的 ProtocolHandler 回调接到会话编排。

use std::io::{self};
use std::sync::Arc;

use async_trait::async_trait;
use p2p::{BoxedStream, ProtocolHandler, ProtocolId};

use crate::session::{serve, SessionDeps};

pub struct AcpHandler {
    deps: Arc<SessionDeps>,
    protocol_id: ProtocolId,
}

impl AcpHandler {
    pub fn new(deps: Arc<SessionDeps>) -> Result<Self, p2p_protocol::ProtocolError> {
        Ok(Self {
            protocol_id: ProtocolId::new(&deps.config.protocol_id)?,
            deps,
        })
    }
}

#[async_trait]
impl ProtocolHandler for AcpHandler {
    fn protocol(&self) -> ProtocolId {
        self.protocol_id.clone()
    }

    async fn handle(&self, stream: BoxedStream) -> io::Result<()> {
        serve(self.deps.clone(), stream).await
    }
}

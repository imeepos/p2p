//! handler 注册表共享单元与 Swarm 工厂句柄。

use std::io;
use std::sync::{Arc, Mutex};

use p2p_identity::PeerId;
use p2p_mux::BoxedStream;
use p2p_protocol::{HandlerRegistry, ProtocolId, StreamFactory};

use super::Swarm;

/// handler 注册表的共享单元：注册为复制-改-换，分发按快照路由。
pub type RegistryCell = Arc<Mutex<Arc<HandlerRegistry>>>;

/// [Swarm] 的可拥有工厂句柄：RequestResponseClient 等需要持有工厂的场景使用。
#[derive(Clone)]
pub struct SwarmFactory(pub Arc<Swarm>);

#[async_trait::async_trait]
impl StreamFactory for SwarmFactory {
    async fn open_stream(&self, peer: &PeerId, protocol: &ProtocolId) -> io::Result<BoxedStream> {
        self.0.open_stream(peer, protocol).await
    }
}

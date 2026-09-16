//! 挂载桥装配：把 [DavService] 绑到本机回环端口，OS 原生 WebDAV 客户端
//! （macOS mount_webdav / Finder、Linux davfs2、Windows net use）直接挂载。
//!
//! 默认只绑 127.0.0.1：桥不增设鉴权面，网络边界由底座（传输加密 + 对端
//! 身份互认）承担，本机边界由回环绑定承担。

use std::sync::Arc;

use tokio::net::TcpListener;

use crate::backend::FsBackend;
use crate::dav::DavService;
use crate::http;

/// 桥配置。
#[derive(Debug, Clone)]
pub struct MountConfig {
    /// 监听地址（默认 127.0.0.1；0.0.0.0 需宿主显式开启并自担鉴权面）。
    pub bind_addr: std::net::IpAddr,
    /// 监听端口（0 = 随机）。
    pub port: u16,
}

impl Default for MountConfig {
    fn default() -> Self {
        Self {
            bind_addr: std::net::IpAddr::from([127, 0, 0, 1]),
            port: 0,
        }
    }
}

/// 已就绪的桥：句柄存活期间持续服务，`shutdown` 或 drop listener 收口。
pub struct MountBridge {
    pub addr: std::net::SocketAddr,
    listener: TcpListener,
}

impl MountBridge {
    /// 绑定端口（装配期显式失败，禁静默换端口）。
    pub async fn bind(cfg: MountConfig) -> std::io::Result<Self> {
        let listener = TcpListener::bind((cfg.bind_addr, cfg.port)).await?;
        let addr = listener.local_addr()?;
        Ok(Self { addr, listener })
    }

    /// 服务到 shutdown 触发；返回前 listener 先关（accept 退出）。
    pub async fn run(
        self,
        backend: Arc<dyn FsBackend>,
        shutdown: tokio::sync::watch::Receiver<bool>,
    ) {
        let service: Arc<dyn http::HttpHandler> = Arc::new(DavService::new(backend));
        http::serve(self.listener, service, shutdown).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn bind_random_port_reports_addr() {
        let bridge = MountBridge::bind(MountConfig::default()).await.unwrap();
        assert_eq!(bridge.addr.ip().to_string(), "127.0.0.1");
        assert!(bridge.addr.port() > 0);
    }
}

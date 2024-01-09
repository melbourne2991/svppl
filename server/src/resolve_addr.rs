use tokio::net::lookup_host;
use std::net::{SocketAddr};
use anyhow::Result;

pub async fn resolve_socket_addr(hostname: &str, port: u16) -> Result<SocketAddr> {
    let ips = lookup_host(hostname).await?;
    
    for addr in ips {
        let mut addr_port = addr.clone();
        addr_port.set_port(port);
        return Ok(addr_port)
    }

    Err(anyhow::anyhow!("Failed to resolve hostname: {}", hostname))
}
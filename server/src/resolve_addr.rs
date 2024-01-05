use dns_lookup::{lookup_host, lookup_addr};
use std::net::{IpAddr, SocketAddr};
use anyhow::Result;

pub fn resolve_socket_addr(hostname: &str, port: u16) -> Result<SocketAddr> {
    let ips = lookup_host(hostname)?;
    
    for ip in ips {
        return Ok(SocketAddr::new(ip, port));
    }

    Err(anyhow::anyhow!("Failed to resolve hostname: {}", hostname))
}
//! Port of `lib/utils/ping.js`: `GET /-/ping` against the registry.

use crate::utils::errors::Result;
use serde_json::{Map, Value};

/// Returns the JSON body of the ping response (usually `{}`).
pub fn ping(registry: &str) -> Result<Map<String, Value>> {
    let ping_url = format!("{}/-/ping", registry.trim_end_matches('/'));
    // TODO: route through the shared registry client once it exists. Opening
    // a TCP connection is enough to detect reachability for now.
    match std::net::TcpStream::connect_timeout(
        &resolve_authority(&ping_url)?,
        std::time::Duration::from_secs(10),
    ) {
        Ok(_) => Ok(Map::new()),
        Err(err) => Err(crate::utils::errors::NpmError::new(format!(
            "ping failed for {ping_url}: {err}"
        ))
        .with_code("ECONNREFUSED")),
    }
}

fn resolve_authority(url: &str) -> Result<std::net::SocketAddr> {
    let parsed =
        url::Url::parse(url).map_err(|err| crate::utils::errors::NpmError::new(err.to_string()))?;
    let host = parsed
        .host_str()
        .ok_or_else(|| crate::utils::errors::NpmError::new("registry url has no host"))?;
    let port = parsed.port_or_known_default().unwrap_or(443);
    use std::net::ToSocketAddrs;
    (host, port)
        .to_socket_addrs()
        .ok()
        .and_then(|mut addrs| addrs.next())
        .ok_or_else(|| {
            crate::utils::errors::NpmError::new(format!("could not resolve {host}"))
                .with_code("ENOTFOUND")
        })
}

//! Automatic inverter discovery.
//!
//! Scans the local network (assuming a /24 subnet) for GivEnergy
//! inverters by probing the known Modbus TCP port (default **8899**).
//!
//! ## How it works
//!
//! 1. Determine the host machine's LAN IPv4 address via
//!    [`local_ip_address`].
//! 2. Derive the /24 subnet from that address.
//! 3. For every host in the range `.1` … `.254`, open a TCP
//!    connection with a short timeout.
//! 4. Hosts that accept the connection are reported as
//!    [`DiscoveredInverter`] entries.
//!
//! Serial-number detection is left as a refinement — v1 simply
//! detects open ports.

use std::net::{Ipv4Addr, SocketAddrV4};
use std::time::Duration;

use tokio::task::JoinSet;

// ---------------------------------------------------------------------------
// Data types
// ---------------------------------------------------------------------------

/// A device discovered on the LAN that accepts connections on the
/// GivEnergy Modbus port.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct DiscoveredInverter {
    /// IP address or hostname of the discovered device.
    pub host: String,
    /// Port on which the TCP connection succeeded (typically 8899).
    pub port: u16,
    /// Serial number, if we managed to read it from the device.
    pub serial: Option<String>,
    /// Inverter generation / model hint, if known.
    pub generation: Option<String>,
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Scan the local network for inverters.
///
/// Tries connecting to `port` on every host in the /24 subnet of the
/// local machine, with each connection attempt capped at `timeout_ms`.
///
/// Returns a (possibly empty) vector of hosts that accepted the TCP
/// connection – very likely GivEnergy inverters.
pub async fn scan_network(port: u16, timeout_ms: u64) -> Vec<DiscoveredInverter> {
    let local_ip = match get_local_ip() {
        Some(ip) => ip,
        None => {
            tracing::warn!("discovery: cannot determine local IP – aborting scan");
            return Vec::new();
        }
    };

    let octets = local_ip.octets();
    let subnet_prefix = [octets[0], octets[1], octets[2]];

    tracing::info!(
        "discovery: scanning {}.{}.{}.*:{} (timeout {} ms)",
        subnet_prefix[0],
        subnet_prefix[1],
        subnet_prefix[2],
        port,
        timeout_ms,
    );

    let timeout = Duration::from_millis(timeout_ms);
    let mut tasks: JoinSet<(u8, Result<(), ()>)> = JoinSet::new();

    // Spawn one task per host address (1..=254).
    for host_num in 1u8..=254 {
        let addr = SocketAddrV4::new(
            Ipv4Addr::new(subnet_prefix[0], subnet_prefix[1], subnet_prefix[2], host_num),
            port,
        );
        let timeout = timeout;
        tasks.spawn(async move {
            match tokio::time::timeout(timeout, tokio::net::TcpStream::connect(addr)).await {
                Ok(Ok(_stream)) => (host_num, Ok(())),
                _ => (host_num, Err(())),
            }
        });
    }

    // Collect successful hits.
    let mut results = Vec::new();
    while let Some(res) = tasks.join_next().await {
        match res {
            Ok((host_num, Ok(()))) => {
                let host = format!(
                    "{}.{}.{}.{}",
                    subnet_prefix[0], subnet_prefix[1], subnet_prefix[2], host_num
                );
                tracing::info!("discovery: found device at {host}:{port}");
                results.push(DiscoveredInverter {
                    host,
                    port,
                    serial: None,
                    generation: None,
                });
            }
            Ok((_host_num, Err(()))) => { /* not reachable – skip */ }
            Err(e) => {
                tracing::debug!("discovery: join error: {e}");
            }
        }
    }

    tracing::info!("discovery: scan complete – {} device(s) found", results.len());
    results
}

/// Get the local machine's LAN IPv4 address.
///
/// Returns `None` if the address cannot be determined (e.g. no
/// suitable network interface, or running in an environment without
/// network access).
pub fn get_local_ip() -> Option<Ipv4Addr> {
    match local_ip_address::local_ip() {
        Ok(std::net::IpAddr::V4(ipv4)) => Some(ipv4),
        Ok(other) => {
            tracing::warn!("discovery: local IP is not IPv4: {other:?}");
            None
        }
        Err(e) => {
            tracing::warn!("discovery: failed to get local IP: {e}");
            None
        }
    }
}

/// Return the LAN HTTP address of this machine (useful for QR codes /
/// displaying to the user).
///
/// Format: `http://<lan-ip>:<server_port>`
pub fn get_lan_address(server_port: u16) -> Option<String> {
    get_local_ip().map(|ip| format!("http://{ip}:{server_port}"))
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Derive the /24 network prefix from an IPv4 address.
pub fn subnet_prefix(ip: &Ipv4Addr) -> [u8; 3] {
    let o = ip.octets();
    [o[0], o[1], o[2]]
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_local_ip_returns_something_or_none() {
        // Integration-style test: we cannot guarantee a LAN interface
        // exists (e.g. in CI), so both outcomes are acceptable.
        let result = get_local_ip();
        match result {
            Some(ip) => {
                assert!(!ip.is_unspecified(), "should not be 0.0.0.0");
                assert!(
                    ip.is_private(),
                    "expected a private/LAN address, got {ip}"
                );
            }
            None => { /* acceptable in headless / CI environments */ }
        }
    }

    #[test]
    fn discovered_inverter_serde() {
        let inv = DiscoveredInverter {
            host: "192.168.1.33".into(),
            port: 8899,
            serial: Some("GE12345".into()),
            generation: Some("GivHybrid".into()),
        };
        let json = serde_json::to_string(&inv).expect("serialize");
        assert!(json.contains("\"host\":\"192.168.1.33\""));
        assert!(json.contains("\"port\":8899"));
        assert!(json.contains("\"serial\":\"GE12345\""));
        assert!(json.contains("\"generation\":\"GivHybrid\""));

        // Round-trip
        let _: DiscoveredInverter = serde_json::from_str(&json).expect("deserialize");
    }

    #[test]
    fn subnet_prefix_calculation() {
        assert_eq!(
            subnet_prefix(&Ipv4Addr::new(192, 168, 0, 1)),
            [192, 168, 0]
        );
        assert_eq!(
            subnet_prefix(&Ipv4Addr::new(10, 0, 0, 255)),
            [10, 0, 0]
        );
        assert_eq!(
            subnet_prefix(&Ipv4Addr::new(172, 16, 42, 100)),
            [172, 16, 42]
        );
    }

    #[test]
    fn get_lan_address_format() {
        // May return None in CI — that is fine.
        if let Some(addr) = get_lan_address(7337) {
            assert!(
                addr.starts_with("http://"),
                "expected http:// prefix, got: {addr}"
            );
            assert!(
                addr.ends_with(":7337"),
                "expected :7337 suffix, got: {addr}"
            );
        }
    }

    #[test]
    fn discovered_inverter_minimal_serde() {
        let inv = DiscoveredInverter {
            host: "10.0.0.1".into(),
            port: 8899,
            serial: None,
            generation: None,
        };
        let json = serde_json::to_string(&inv).expect("serialize");
        assert!(json.contains("\"serial\":null"));
        assert!(json.contains("\"generation\":null"));
    }
}

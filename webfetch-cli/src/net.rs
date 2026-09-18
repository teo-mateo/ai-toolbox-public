use anyhow::{anyhow, Result};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, ToSocketAddrs};
use url::Host;

/// Mirror of Python ipaddress._is_blocked_ip.
pub fn is_blocked_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            v4.is_private()
                || v4.is_loopback()
                || v4.is_link_local()
                || v4.is_multicast()
                || v4.is_unspecified()
                || v4.is_broadcast()
                || reserved_v4(&v4)
        }
        IpAddr::V6(v6) => {
            v6.is_loopback()
                || v6.is_multicast()
                || v6.is_unspecified()
                || v6.is_unique_local()
                || v6.is_unicast_link_local()
                || reserved_v6(&v6)
        }
    }
}

/// IPv4 reserved ranges not covered by std: 0.0.0.0/8 and 240.0.0.0/4 (class E).
fn reserved_v4(v4: &Ipv4Addr) -> bool {
    let o = v4.octets();
    o[0] == 0 || o[0] >= 240
}

/// IPv6 reserved ranges not covered by std (mirrors Python ipaddress.is_reserved).
fn reserved_v6(v6: &Ipv6Addr) -> bool {
    let s = v6.segments();
    // ::ffff:0:0/96 (IPv4-mapped)
    (s[0..5].iter().all(|&x| x == 0) && s[5] == 0xffff)
    // 2001:db8::/32 (documentation)
    || (s[0] == 0x2001 && s[1] == 0x0db8)
    // 64:ff9b::/96 (NAT64 well-known prefix)
    || (s[0] == 0x0064 && s[1] == 0xff9b && s[2..5].iter().all(|&x| x == 0))
    // 100::/64 (discard)
    || (s[0] == 0x0100 && s[1..4].iter().all(|&x| x == 0))
    // 2001:10::/28 (ORCHID)
    || (s[0] == 0x2001 && (s[1] & 0xfff0) == 0x0010)
    // 2001:20::/28 (ORCHIDv2)
    || (s[0] == 0x2001 && (s[1] & 0xfff0) == 0x0020)
    // 2001:2::/48 (benchmarking)
    || (s[0] == 0x2001 && s[1] == 0x0002 && s[2] == 0)
    // 2001::/23 (reserved)
    || (s[0] == 0x2001 && (s[1] & 0xffe0) == 0)
    // 2001:1::1/128
    || (s[0] == 0x2001 && s[1] == 0x0001 && s[2] == 0 && s[3] == 0
        && s[4] == 0 && s[5] == 0 && s[6] == 0 && s[7] == 1)
    // ::1:ffff:0:0/96
    || (s[0..6].iter().all(|&x| x == 0) && s[6] == 0xffff)
    // 64:ff9b:1::/48
    || (s[0] == 0x0064 && s[1] == 0xff9b && s[2] == 1)
    // 100:64::/10
    || ((s[0] & 0xffc0) == 0x0100 && s[1] == 0x0040)
    // 5f00::/16 (SRv6)
    || (s[0] == 0x5f00)
    // 3fff::/20 (documentation)
    || ((s[0] & 0xfff0) == 0x3ff0)
}

/// Validate the URL itself: scheme, hostname presence, literal-IP check,
/// and the localhost check. Mirrors Python `_validate_url`.
pub fn validate_url(url_str: &str) -> Result<()> {
    let parsed = url::Url::parse(url_str)
        .map_err(|e| anyhow!("Invalid URL: {}", e))?;
    match parsed.scheme() {
        "http" | "https" => {}
        other => return Err(anyhow!("Blocked URL with disallowed protocol '{}'.", other)),
    }
    match parsed.host() {
        Some(Host::Ipv4(ip)) => {
            if is_blocked_ip(IpAddr::V4(ip)) {
                return Err(anyhow!("Blocked private/local address '{}'.", ip));
            }
        }
        Some(Host::Ipv6(ip)) => {
            if is_blocked_ip(IpAddr::V6(ip)) {
                return Err(anyhow!("Blocked private/local address '{}'.", ip));
            }
        }
        Some(Host::Domain(domain)) => {
            if domain.eq_ignore_ascii_case("localhost") {
                return Err(anyhow!("Blocked localhost URL."));
            }
        }
        None => return Err(anyhow!("URL must include a hostname.")),
    }
    Ok(())
}

/// Resolve a domain hostname and block it if ANY resolved IP is private/local.
/// Mirrors Python `_validate_resolved_ips` (resolution failure → proceed).
pub fn validate_resolved_ips(hostname: &str) -> Result<()> {
    match (hostname, 0).to_socket_addrs() {
        Ok(addrs) => {
            for addr in addrs {
                if is_blocked_ip(addr.ip()) {
                    return Err(anyhow!(
                        "Blocked hostname '{}' because it resolves to private/local IP '{}'.",
                        hostname,
                        addr.ip()
                    ));
                }
            }
            Ok(())
        }
        Err(_) => Ok(()), // resolution failure → let the request fail naturally
    }
}

/// Full pre-request validation: URL checks + DNS resolution check.
pub fn validate_fetch_url(url: &str) -> Result<()> {
    validate_url(url)?;
    let parsed = url::Url::parse(url)?;
    if let Some(Host::Domain(domain)) = parsed.host() {
        validate_resolved_ips(&domain)?;
    }
    Ok(())
}

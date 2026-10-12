// SPDX-FileCopyrightText: (C) 2026 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

//! The client address of a request, behind a reverse proxy or not.

use std::net::{IpAddr, SocketAddr};

use axum::http::HeaderMap;

/// The client IP of a request. With `http.reverse-proxy` enabled this is
/// the address the proxy forwarded: `X-Real-Ip`, or else the last
/// `X-Forwarded-For` entry, the one the proxy itself added. A missing or
/// invalid header falls back to the TCP peer.
pub(crate) fn client_ip(reverse_proxy: bool, headers: &HeaderMap, peer: SocketAddr) -> IpAddr {
    reverse_proxy
        .then(|| forwarded_ip(headers))
        .flatten()
        .unwrap_or(peer.ip())
}

fn forwarded_ip(headers: &HeaderMap) -> Option<IpAddr> {
    let header = |name: &str| headers.get(name).and_then(|value| value.to_str().ok());
    header("x-real-ip").and_then(parse_ip).or_else(|| {
        header("x-forwarded-for")
            .and_then(|value| value.rsplit(',').next())
            .and_then(parse_ip)
    })
}

/// An IP address, possibly with a port, as proxies may send it.
fn parse_ip(value: &str) -> Option<IpAddr> {
    let value = value.trim();
    value
        .parse::<IpAddr>()
        .ok()
        .or_else(|| value.parse::<SocketAddr>().ok().map(|addr| addr.ip()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn headers(pairs: &[(&'static str, &str)]) -> HeaderMap {
        let mut headers = HeaderMap::new();
        for (name, value) in pairs {
            headers.insert(*name, value.parse().unwrap());
        }
        headers
    }

    const PEER: &str = "172.18.0.2:41000";

    fn ip(value: &str) -> IpAddr {
        value.parse().unwrap()
    }

    #[test]
    fn peer_without_reverse_proxy() {
        let headers = headers(&[("x-real-ip", "203.0.113.5")]);
        assert_eq!(
            client_ip(false, &headers, PEER.parse().unwrap()),
            ip("172.18.0.2")
        );
    }

    #[test]
    fn forwarded_with_reverse_proxy() {
        let peer = PEER.parse().unwrap();
        let real_ip = headers(&[
            ("x-real-ip", "203.0.113.5"),
            ("x-forwarded-for", "198.51.100.1"),
        ]);
        assert_eq!(client_ip(true, &real_ip, peer), ip("203.0.113.5"));

        // The last entry is the one the proxy added; earlier ones came
        // from the client and can be anything.
        let chain = headers(&[("x-forwarded-for", "10.9.9.9, 203.0.113.5")]);
        assert_eq!(client_ip(true, &chain, peer), ip("203.0.113.5"));

        let with_port = headers(&[("x-forwarded-for", "[2001:db8::1]:443")]);
        assert_eq!(client_ip(true, &with_port, peer), ip("2001:db8::1"));
    }

    #[test]
    fn invalid_or_missing_falls_back_to_peer() {
        let peer = PEER.parse().unwrap();
        let invalid = headers(&[("x-real-ip", "not an ip"), ("x-forwarded-for", "")]);
        assert_eq!(client_ip(true, &invalid, peer), ip("172.18.0.2"));
        assert_eq!(client_ip(true, &HeaderMap::new(), peer), ip("172.18.0.2"));
    }
}

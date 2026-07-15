// SPDX-FileCopyrightText: (C) 2026 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

#![cfg_attr(windows, allow(dead_code))]

//! URL and TLS helpers for the agent's WebSocket control channel.
//!
//! Normal connections use tokio-tungstenite's native-root rustls connector.
//! When the existing agent `disable-certificate-check` option is enabled, the
//! custom connector skips certificate chain, name, and expiry validation but
//! still verifies the handshake signature against the presented key.

use std::sync::Arc;

use anyhow::Context;
use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::crypto::{
    WebPkiSupportedAlgorithms, ring, verify_tls12_signature, verify_tls13_signature,
};
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use rustls::{DigitallySignedStruct, SignatureScheme};
use tokio_tungstenite::Connector;

use super::protocol::AGENT_WS_PATH;

/// Normalize the configured EveBox server URL shared by the WebSocket
/// control plane and HTTP upload data plane.
///
/// Only ordinary HTTP(S) server URLs are valid. Query/fragment components do
/// not identify the EveBox deployment and are discarded; a base path is kept
/// without a trailing slash so every endpoint can append its path uniformly.
pub(crate) fn normalize_server_url(server_url: &str) -> anyhow::Result<String> {
    let server_url = server_url.trim();
    let mut url = reqwest::Url::parse(server_url)
        .with_context(|| format!("invalid EveBox server URL {server_url:?}"))?;
    match url.scheme() {
        "http" | "https" => {}
        other => bail!(
            "unsupported EveBox server URL scheme {other:?}; server.url must use http or https"
        ),
    }
    let path = url.path().trim_end_matches('/').to_string();
    url.set_path(&path);
    url.set_query(None);
    url.set_fragment(None);
    Ok(url.to_string().trim_end_matches('/').to_string())
}

/// Convert a normalized EveBox HTTP(S) base URL to its agent WebSocket
/// endpoint while preserving any configured base path.
pub(crate) fn websocket_url(server_url: &str) -> anyhow::Result<String> {
    let normalized = normalize_server_url(server_url)?;
    let mut url = reqwest::Url::parse(&normalized)
        .with_context(|| format!("invalid normalized EveBox server URL {normalized:?}"))?;
    let scheme = match url.scheme() {
        "http" => "ws",
        "https" => "wss",
        _ => unreachable!("normalize_server_url accepts only HTTP(S)"),
    };
    url.set_scheme(scheme)
        .map_err(|()| anyhow!("could not set WebSocket URL scheme"))?;
    let base_path = url.path().trim_end_matches('/');
    url.set_path(&format!("{base_path}{AGENT_WS_PATH}"));
    Ok(url.into())
}

/// Select the TLS connector for the agent control channel.
///
/// `None` tells tokio-tungstenite to use its normal native-root verifier.
/// Certificate checking is disabled only when explicitly requested, matching
/// the existing agent HTTP client's `danger_accept_invalid_certs` behavior.
pub(crate) fn connector(disable_certificate_check: bool) -> Option<Connector> {
    disable_certificate_check.then(|| Connector::Rustls(Arc::new(insecure_client_config())))
}

fn insecure_client_config() -> rustls::ClientConfig {
    rustls::ClientConfig::builder_with_provider(Arc::new(ring::default_provider()))
        .with_safe_default_protocol_versions()
        .expect("ring provides safe default protocol versions")
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(NoVerifier::new()))
        .with_no_client_auth()
}

#[derive(Debug)]
struct NoVerifier {
    supported: WebPkiSupportedAlgorithms,
}

impl NoVerifier {
    fn new() -> Self {
        Self {
            supported: ring::default_provider().signature_verification_algorithms,
        }
    }
}

impl ServerCertVerifier for NoVerifier {
    fn verify_server_cert(
        &self,
        _end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> Result<ServerCertVerified, rustls::Error> {
        Ok(ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        verify_tls12_signature(message, cert, dss, &self.supported)
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        verify_tls13_signature(message, cert, dss, &self.supported)
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.supported.supported_schemes()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_http_and_https_urls() {
        assert_eq!(
            websocket_url("http://localhost:5636").unwrap(),
            "ws://localhost:5636/api/agent/ws"
        );
        assert_eq!(
            websocket_url("https://evebox.example").unwrap(),
            "wss://evebox.example/api/agent/ws"
        );
    }

    #[test]
    fn normalizes_a_base_path_and_discards_query_and_fragment() {
        assert_eq!(
            normalize_server_url("  https://example.test/evebox///?old=yes#fragment  ").unwrap(),
            "https://example.test/evebox"
        );
        assert_eq!(
            websocket_url("https://example.test/evebox///?old=yes#fragment").unwrap(),
            "wss://example.test/evebox/api/agent/ws"
        );
        assert_eq!(
            normalize_server_url("http://localhost:5636/").unwrap(),
            "http://localhost:5636"
        );
    }

    #[test]
    fn rejects_non_http_server_url_schemes() {
        for url in [
            "ws://localhost:5636/",
            "wss://localhost:5636/",
            "ftp://localhost/evebox",
            "localhost:5636",
        ] {
            assert!(websocket_url(url).is_err(), "unexpectedly accepted {url}");
        }
    }

    #[test]
    fn verifying_path_uses_the_default_connector() {
        assert!(connector(false).is_none());
    }

    #[test]
    fn disabled_certificate_check_supplies_a_rustls_connector() {
        assert!(matches!(connector(true), Some(Connector::Rustls(_))));
    }

    #[test]
    fn insecure_verifier_advertises_signature_schemes() {
        let _config = insecure_client_config();
        assert!(!NoVerifier::new().supported_verify_schemes().is_empty());
    }
}

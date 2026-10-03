// SPDX-FileCopyrightText: (C) 2026 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

//! Helpers for building outbound HTTP clients.

/// True if `url` is a plain `http://` URL, so a client for it will never
/// negotiate TLS.
pub(crate) fn is_plain_http(url: &str) -> bool {
    url.trim_start()
        .get(..7)
        .is_some_and(|scheme| scheme.eq_ignore_ascii_case("http://"))
}

/// Start a `reqwest::ClientBuilder` for a client that will talk to `url`.
///
/// For plain `http://` URLs the system CA store is not consulted. With
/// reqwest 0.13 the default (platform) verifier eagerly scans the system
/// CA directories when the client is built, which warns about every file
/// it cannot read (for example a root-only `localhost.crt` in
/// `/etc/pki/tls/certs` on RHEL) and fails the build outright if no roots
/// could be loaded at all. Neither has any bearing on a client that will
/// never do TLS, so use an empty, explicit root set instead. Should such a
/// client ever end up on a TLS connection (an unexpected redirect, say),
/// verification fails closed rather than trusting anything.
pub(crate) fn client_builder(url: &str) -> reqwest::ClientBuilder {
    let builder = reqwest::Client::builder();
    if is_plain_http(url) {
        builder.tls_certs_only(std::iter::empty())
    } else {
        builder
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_http_detection() {
        assert!(is_plain_http("http://localhost:9200"));
        assert!(is_plain_http("HTTP://localhost:9200"));
        assert!(is_plain_http(" http://localhost"));
        assert!(!is_plain_http("https://localhost:9200"));
        assert!(!is_plain_http("localhost:9200"));
        assert!(!is_plain_http("http:/"));
        assert!(!is_plain_http(""));
    }

    #[test]
    fn plain_http_client_builds_with_empty_root_set() {
        let _ = rustls::crypto::ring::default_provider().install_default();
        assert!(client_builder("http://127.0.0.1:1").build().is_ok());
    }
}

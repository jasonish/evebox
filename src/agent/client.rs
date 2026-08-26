// SPDX-FileCopyrightText: (C) 2020 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

/// Build a reqwest client honoring the agent's certificate-check option.
/// Shared by the event importer and the packet-capture upload channel so the
/// TLS policy cannot drift between them.
pub(crate) fn build_reqwest_client(
    disable_certificate_validation: bool,
) -> Result<reqwest::Client, reqwest::Error> {
    // Never follow redirects: the agent key travels in a custom header that
    // reqwest would not strip on a cross-origin redirect, and redirected
    // POST bodies (events, pcap uploads) are not useful anyway.
    let mut builder = reqwest::Client::builder().redirect(reqwest::redirect::Policy::none());
    if disable_certificate_validation {
        builder = builder.danger_accept_invalid_certs(true);
    }
    builder.build()
}

// EveBox agent client (to EveBox server)
#[derive(Clone, Debug)]
pub(crate) struct Client {
    url: String,
    disable_certificate_validation: bool,
    username: Option<String>,
    password: Option<String>,
    agent_key: Option<String>,
}

impl Client {
    pub fn new(
        url: &str,
        username: Option<String>,
        password: Option<String>,
        agent_key: Option<String>,
        disable_certificate_validation: bool,
    ) -> Self {
        Self {
            url: url.to_string(),
            disable_certificate_validation,
            username,
            password,
            agent_key,
        }
    }

    pub fn get_http_client(&self) -> Result<reqwest::Client, reqwest::Error> {
        build_reqwest_client(self.disable_certificate_validation)
    }

    pub fn post(&self, path: &str) -> Result<reqwest::RequestBuilder, reqwest::Error> {
        let url = format!("{}/{}", self.url, path);
        let mut request = self
            .get_http_client()?
            .post(url)
            .header("Content-Type", "application/json");
        if let Some(key) = &self.agent_key {
            request = request.header(crate::agent::protocol::AGENT_KEY_HEADER, key);
        }
        let request = if let Some(username) = &self.username {
            request.basic_auth(username, self.password.clone())
        } else {
            request
        };
        Ok(request)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn agent_key_does_not_displace_basic_auth() {
        let _ = rustls::crypto::ring::default_provider().install_default();
        let client = Client::new(
            "https://evebox.test",
            Some("legacy-user".to_string()),
            Some("legacy-password".to_string()),
            Some("eba_test".to_string()),
            false,
        );
        let request = client.post("api/submit").unwrap().build().unwrap();
        assert_eq!(
            request
                .headers()
                .get(crate::agent::protocol::AGENT_KEY_HEADER)
                .unwrap(),
            "eba_test"
        );
        assert_eq!(
            request.headers().get("authorization").unwrap(),
            "Basic bGVnYWN5LXVzZXI6bGVnYWN5LXBhc3N3b3Jk"
        );
    }
}

// SPDX-License-Identifier: MIT
//
// Copyright (C) 2020-2022 Jason Ish

// EveBox agent client (to EveBox server)
#[derive(Clone, Debug)]
pub struct Client {
    url: String,
    disable_certificate_validation: bool,
    username: Option<String>,
    password: Option<String>,
}

impl Client {
    pub fn new(
        url: &str,
        username: Option<String>,
        password: Option<String>,
        disable_certificate_validation: bool,
    ) -> Self {
        Self {
            url: url.to_string(),
            disable_certificate_validation,
            username,
            password,
        }
    }

    pub fn get_http_client(&self) -> Result<reqwest::Client, reqwest::Error> {
        let mut builder = reqwest::Client::builder();
        if self.disable_certificate_validation {
            builder = builder.danger_accept_invalid_certs(true);
        }
        builder.build()
    }

    pub fn post(&self, path: &str) -> Result<reqwest::RequestBuilder, reqwest::Error> {
        let url = format!("{}/{}", self.url, path);
        let request = self
            .get_http_client()?
            .post(url)
            .header("Content-Type", "application/json");
        let request = if let Some(username) = &self.username {
            request.basic_auth(username, self.password.clone())
        } else {
            request
        };
        Ok(request)
    }
}

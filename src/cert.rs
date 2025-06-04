// SPDX-FileCopyrightText: (C) 2023 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

use anyhow::Result;
use rcgen::{CertificateParams, DistinguishedName, DnType, Ia5String, KeyPair};
use std::fs;
use std::path::{Path, PathBuf};
use tracing::info;

static ORG_NAME: &str = "EveBox Server";
static CN_NAME: &str = "EveBox Server";
static CERT_FILENAME: &str = "cert.pem";
static KEY_FILENAME: &str = "key.pem";

pub(crate) fn create_and_write_cert<P: AsRef<Path>>(dir: P) -> Result<(PathBuf, PathBuf)> {
    let mut params: CertificateParams = Default::default();
    params.not_before = rcgen::date_time_ymd(2023, 1, 1);
    params.not_after = rcgen::date_time_ymd(3023, 1, 1);
    params.distinguished_name = DistinguishedName::new();
    params
        .distinguished_name
        .push(DnType::OrganizationName, ORG_NAME);
    params.distinguished_name.push(DnType::CommonName, CN_NAME);
    params.subject_alt_names = vec![rcgen::SanType::DnsName(Ia5String::try_from(
        "localhost".to_string(),
    )?)];
    let key_pair = KeyPair::generate()?;
    let cert = params.self_signed(&key_pair)?;
    let dir = dir.as_ref();
    let cert_path = dir.join(CERT_FILENAME);
    let key_path = dir.join(KEY_FILENAME);
    fs::write(&cert_path, cert.pem().as_bytes())?;
    fs::write(&key_path, key_pair.serialize_pem().as_bytes())?;
    Ok((cert_path, key_path))
}

pub(crate) fn get_or_create_cert<P: AsRef<Path>>(dir: P) -> Result<(PathBuf, PathBuf)> {
    let dir = dir.as_ref();
    let cert_path = dir.join(CERT_FILENAME);
    let key_path = dir.join(KEY_FILENAME);

    if cert_path.exists() && key_path.exists() {
        info!(
            "Found existing TLS certificate and key: {}, {}",
            cert_path.display(),
            key_path.display()
        );
        Ok((cert_path, key_path))
    } else {
        let (cert_path, key_path) = create_and_write_cert(dir)?;
        info!(
            "Created new TLS certificate and key: {}, {}",
            cert_path.display(),
            key_path.display()
        );
        Ok((cert_path, key_path))
    }
}

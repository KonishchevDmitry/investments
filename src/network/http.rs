use std::time::Instant;

use lazy_static::lazy_static;
use log::debug;
use reqwest::Certificate;
use reqwest::blocking::{Client, Response};

use crate::core::GenericResult;
use crate::util;

use super::certificates;

pub fn client_with_russian_root_ca() -> GenericResult<Client> {
    lazy_static! {
        static ref RUSSIAN_TRUSTED_ROOT_CA: GenericResult<Certificate> = {
            Ok(Certificate::from_pem(certificates::RUSSIAN_TRUSTED_ROOT_CA)?)
        };
    }

    let russian_trusted_root_ca = RUSSIAN_TRUSTED_ROOT_CA.as_ref().map_err(|e| format!(
        "Failed to load bundled Russian Trusted Root CA: {e}"))?;

    Ok(Client::builder().tls_certs_merge([russian_trusted_root_ca.clone()]).build()?)
}

pub fn send_request<U: AsRef<str>>(client: &Client, url: U, authorization: Option<&str>) -> GenericResult<Response> {
    let url = url.as_ref();

    let mut request = client.get(url);
    if let Some(authorization) = authorization {
        request = request.bearer_auth(authorization);
    }

    debug!("Sending request to {url}...");
    let start = Instant::now();
    let response = request.send().map_err(util::humanize_reqwest_error)?;
    let duration = start.elapsed();
    debug!("Got response from {url} ({duration:?}).");

    if !response.status().is_success() {
        return Err!("Server returned an error: {}", response.status());
    }

    Ok(response)
}
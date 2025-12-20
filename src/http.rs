use std::time::Instant;

use log::debug;
use reqwest::blocking::{Client, Response};

use crate::core::GenericResult;
use crate::util;

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
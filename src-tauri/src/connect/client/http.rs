//! Thin helpers over `reqwest` shared by the SSO and API clients.

use std::sync::LazyLock;

use bytes::Bytes;
use reqwest::{RequestBuilder, StatusCode};

use super::error::Result;

/// Shared client without a cookie store, for requests that must not carry session cookies.
static CLIENT: LazyLock<reqwest::Client> = LazyLock::new(reqwest::Client::new);

pub fn client() -> &'static reqwest::Client {
    &CLIENT
}

/// Status and fully read body of a response.
pub struct Response {
    pub status: StatusCode,
    pub body: Bytes,
}

impl Response {
    pub fn text(&self) -> String {
        String::from_utf8_lossy(&self.body).into_owned()
    }
}

/// Send `request` and read the whole body.
pub async fn send(request: RequestBuilder) -> Result<Response> {
    let response = request.send().await?;
    let status = response.status();
    let body = response.bytes().await?;
    Ok(Response { status, body })
}

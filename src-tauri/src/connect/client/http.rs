//! Thin helpers over `reqwest` shared by the SSO and API clients.

use bytes::Bytes;
use reqwest::{RequestBuilder, StatusCode};

use super::error::Result;

pub use crate::utils::http::client;

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

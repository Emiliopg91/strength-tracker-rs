//! Async wrapper over the blocking `curl_rest` client, plus a minimal cookie store.

use std::collections::BTreeMap;
use std::sync::Mutex;

use curl_rest::{Client, Header, Response, StatusCode};

use super::error::Result;

/// Same redirect limit reqwest applies by default
const MAX_REDIRECTS: i8 = 10;

/// Run `request` against `url` on the blocking thread pool.
pub async fn send(request: Client<'static>, url: String) -> Result<Response> {
    let response =
        tokio::task::spawn_blocking(move || request.max_redirects(MAX_REDIRECTS).send(&url))
            .await??;
    Ok(response)
}

pub fn is_success(status: StatusCode) -> bool {
    (200..300).contains(&status.as_u16())
}

pub fn body_text(response: &Response) -> String {
    String::from_utf8_lossy(&response.body).into_owned()
}

/// Name/value cookie store for a single host; attributes (path, expiry...) are ignored.
#[derive(Default)]
pub struct CookieStore {
    cookies: Mutex<BTreeMap<String, String>>,
}

impl CookieStore {
    /// `Cookie` header carrying every stored cookie, if any.
    pub fn header(&self) -> Option<Header<'static>> {
        let cookies = self.cookies.lock().unwrap();
        if cookies.is_empty() {
            return None;
        }
        let value = cookies
            .iter()
            .map(|(name, value)| format!("{}={}", name, value))
            .collect::<Vec<_>>()
            .join("; ");
        Some(Header::Custom("Cookie".into(), value.into()))
    }

    /// Store the `Set-Cookie` headers of `response`, including those of followed redirects.
    pub fn store(&self, response: &Response) {
        let mut cookies = self.cookies.lock().unwrap();
        for header in &response.headers {
            if !header.name.eq_ignore_ascii_case("Set-Cookie") {
                continue;
            }
            let pair = header.value.split(';').next().unwrap_or_default();
            if let Some((name, value)) = pair.split_once('=') {
                cookies.insert(name.trim().to_string(), value.trim().to_string());
            }
        }
    }
}

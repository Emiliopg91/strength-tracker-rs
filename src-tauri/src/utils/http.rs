use std::sync::LazyLock;

/// HTTP client shared by the whole app, so requests to the same host reuse its pooled connections
/// and TLS sessions. It has no cookie store, so requests carry no session cookies.
static CLIENT: LazyLock<reqwest::Client> = LazyLock::new(reqwest::Client::new);

pub fn client() -> &'static reqwest::Client {
    &CLIENT
}

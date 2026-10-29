# Garmin Connect via `garmin-cli`

How to use the [`garmin-cli`](https://github.com/vicentereig/garmin-cli) crate (v2.2.0, MIT) as a
library to log in to Garmin Connect, list activities and download their original `.FIT` files.

> Garmin Connect has no public API for individuals. This crate talks to the same
> undocumented endpoints the mobile app uses, so it can break whenever Garmin changes
> its auth flow (it happened in March and April 2026). Verified working on 2026-10-29.

## Dependency

```toml
[dependencies]
garmin-cli = "=2.2.0"
zip = "2"   # the FIT download is a zip archive
```

The crate has no feature flags: the library pulls in everything the CLI uses
(`arrow`, `parquet`, `rusqlite`, `clap`, `keyring`, `gpx`, ...). Only the `client`
module is needed here. If the dependency weight matters, vendor
`src/client/{sso,oauth1,tokens,api}.rs` plus `src/error.rs` instead (MIT licensed).

It uses `reqwest 0.12` with `rustls-tls` and `tokio`.

## Public API used

| Item | Purpose |
|---|---|
| `garmin_cli::client::SsoClient` | SSO login and OAuth2 refresh |
| `garmin_cli::client::GarminClient` | Authenticated calls to `connectapi.garmin.com` |
| `garmin_cli::client::{OAuth1Token, OAuth2Token}` | Tokens (both `Serialize + Deserialize`) |
| `garmin_cli::models::ActivitySummary` | One entry of the activity list |
| `garmin_cli::{GarminError, Result}` | Error type |

```rust
impl SsoClient {
    pub fn new() -> Result<Self>;                                   // garmin.com
    pub fn with_domain(domain: impl Into<String>) -> Result<Self>;  // "garmin.com" | "garmin.cn"
    pub async fn login(&mut self, email: &str, password: &str,
                       mfa_callback: Option<impl FnOnce() -> String>)
        -> Result<(OAuth1Token, OAuth2Token)>;
    pub async fn refresh_oauth2(&self, oauth1: &OAuth1Token) -> Result<OAuth2Token>;
}

impl GarminClient {
    pub fn new() -> Self;   // base URL from GARMIN_DOMAIN env var, default garmin.com
    pub async fn get_json<T: DeserializeOwned>(&self, token: &OAuth2Token, path: &str) -> Result<T>;
    pub async fn download(&self, token: &OAuth2Token, path: &str) -> Result<Bytes>;
}
```

## Tokens

| Token | Lifetime | Use |
|---|---|---|
| `OAuth1Token` | ~1 year | Obtained once at login (needs password and MFA). Only used to mint OAuth2 tokens. |
| `OAuth2Token` | Short (`expires_in`, absolute `expires_at`) | `Authorization: Bearer` header on every API call. |

- Refreshing an expired OAuth2 token only needs the OAuth1 token (`refresh_oauth2`), no password.
- A new login is only needed when the OAuth1 token expires or Garmin revokes it.
- Persist both tokens between runs. Store them in the system keyring rather than plain JSON:
  the OAuth1 token gives full account access for a year.
- `OAuth2Token::is_expired()` compares `expires_at` against now. `expires_at` defaults to `0`
  when missing, so a token without it is treated as expired.

## 1. Login

```rust
use garmin_cli::client::{OAuth1Token, OAuth2Token, SsoClient};

async fn login(email: &str, password: &str) -> garmin_cli::Result<(OAuth1Token, OAuth2Token)> {
    let mut sso = SsoClient::new()?;
    // Called only if Garmin answers MFA_REQUIRED. Must return the code synchronously.
    let mfa = || read_mfa_code_from_user();
    sso.login(email, password, Some(mfa)).await
}
```

Pass `None::<fn() -> String>` to disable MFA. The login then fails with
`GarminError::MfaRequired` if the account needs it.

The callback is `FnOnce() -> String` and blocks inside the async login. In a GUI (Tauri),
either block on a channel that the frontend fills, or ask for the code beforehand.

Flow performed by `login`:

1. `GET  sso.garmin.com/mobile/sso/en/sign-in?clientId=GCM_ANDROID_DARK` (cookies)
2. `POST sso.garmin.com/mobile/api/login` → service ticket, or an MFA challenge
3. `POST sso.garmin.com/mobile/api/mfa/verifyCode` (only with MFA)
4. `GET  connectapi.garmin.com/oauth-service/oauth/preauthorized` (OAuth1-signed) → `OAuth1Token`
5. `POST connectapi.garmin.com/oauth-service/oauth/exchange/user/2.0` (OAuth1-signed) → `OAuth2Token`

The OAuth consumer key and secret (from the Garmin Connect Android app) are hardcoded in
`sso.rs`. Only if a signed request fails with them are they re-downloaded from
`https://thegarth.s3.amazonaws.com/oauth_consumer.json` (a third-party bucket from the
discontinued `garth` project) and the request retried, provided the remote values differ.

## 2. Getting a valid token

This mirrors `garmin_cli::cli::commands::auth::refresh_token`:

```rust
async fn valid_oauth2(oauth1: &OAuth1Token, oauth2: OAuth2Token)
    -> garmin_cli::Result<OAuth2Token>
{
    if !oauth2.is_expired() {
        return Ok(oauth2);
    }
    let sso = SsoClient::with_domain(oauth1.domain.clone())?;
    let fresh = sso.refresh_oauth2(oauth1).await?;
    // persist `fresh` here
    Ok(fresh)
}
```

Call it before each batch of API requests.

## 3. Listing activities

```rust
use garmin_cli::client::GarminClient;
use garmin_cli::models::ActivitySummary;

async fn list(token: &OAuth2Token, start: u32, limit: u32)
    -> garmin_cli::Result<Vec<ActivitySummary>>
{
    let path = format!(
        "/activitylist-service/activities/search/activities?limit={limit}&start={start}"
    );
    GarminClient::new().get_json(token, &path).await
}
```

- Results come newest first. Page with `start += limit` until a page comes back empty.
- The endpoint also accepts `startDate=YYYY-MM-DD&endDate=YYYY-MM-DD` to filter by date.
- Useful `ActivitySummary` fields (all `Option` except `activity_id`): `activity_id: u64`,
  `activity_name`, `start_time_local`, `start_time_gmt` (ISO 8601 strings),
  `activity_type.type_key` (`"running"`, `"strength_training"`, ...), `distance` (m),
  `duration` / `elapsed_duration` / `moving_duration` (s), `calories`, `average_hr`, `max_hr`,
  `elevation_gain`, `steps`.
- For fields the struct does not map, deserialize into `serde_json::Value` instead.

## 4. Downloading the original FIT

```rust
use std::io::{Cursor, Read};

async fn download_fit(token: &OAuth2Token, activity_id: u64) -> anyhow::Result<Vec<u8>> {
    let path = format!("/download-service/files/activity/{activity_id}");
    let zipped = GarminClient::new().download(token, &path).await?;

    let mut archive = zip::ZipArchive::new(Cursor::new(zipped))?;
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i)?;
        if entry.name().to_ascii_lowercase().ends_with(".fit") {
            let mut fit = Vec::with_capacity(entry.size() as usize);
            entry.read_to_end(&mut fit)?;
            return Ok(fit);
        }
    }
    anyhow::bail!("no .fit entry in archive for activity {activity_id}")
}
```

The archive normally contains a single `<activity_id>_ACTIVITY.fit`. This is the same file
the watch produces, so it can go straight into the existing `rustyfit` parser.

Other formats (converted by Garmin, lossy compared to FIT):

| Format | Path |
|---|---|
| FIT (zip) | `/download-service/files/activity/{id}` |
| GPX | `/download-service/export/gpx/activity/{id}` |
| TCX | `/download-service/export/tcx/activity/{id}` |
| KML | `/download-service/export/kml/activity/{id}` |

## Errors to handle

| `GarminError` | Meaning | Action |
|---|---|---|
| `MfaRequired` | Account needs MFA and no callback was given | Ask for the code and retry the login |
| `Authentication(_)` | Bad credentials or a rejected ticket or token | Ask the user to log in again |
| `RateLimited` | HTTP 429 from SSO or the API | Back off. Do not retry in a loop: Garmin/Cloudflare escalate blocks |
| `NotAuthenticated` | HTTP 401 on an API call: OAuth2 rejected | Refresh once. If it still fails, log in again |
| `Api { status: 403, .. }` | Usually Cloudflare blocking the client's TLS fingerprint | Nothing to do client-side with `reqwest` |
| `NotFound(_)` | Unknown activity id | Skip it |

Keep request volume low: logins especially, but also bulk downloads. Persist tokens instead of
logging in on every run.

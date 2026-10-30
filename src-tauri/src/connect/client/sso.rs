//! Garmin SSO Authentication
//!
//! Implements the Garmin Connect SSO login flow using the mobile JSON API.
//!
//! ## Background
//!
//! The previous implementation scraped `<title>` tags from Garmin's SSO HTML
//! page to determine login status (matching `garth < 0.8.0`). Garmin changed
//! their SSO page structure, breaking this approach.
//!
//! This implementation ports the approach from `garth 0.8.0`, which switched
//! to Garmin's mobile JSON API endpoints:
//! - `GET  /mobile/sso/en/sign-in` — establish session cookies
//! - `POST /mobile/api/login` — submit credentials, receive ticket or MFA challenge
//! - `POST /mobile/api/mfa/verifyCode` — verify MFA code if required
//!
//! Reference: <https://github.com/matin/garth/blob/main/garth/sso.py>

use super::GLOBAL_DOMAIN;
use super::error::{GarminError, Result};
use super::http::{self, CookieStore};
use super::oauth1::{OAuth1Signer, OAuthConsumer, OAuthToken, parse_oauth_response};
use super::tokens::{OAuth1Token, OAuth2Token};
use curl_rest::{Client, Header, Response, StatusCode};
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

/// Client ID for mobile API login
const CLIENT_ID: &str = "GCM_ANDROID_DARK";

/// User agent mimicking the Garmin mobile app (for OAuth endpoints)
const MOBILE_USER_AGENT: &str = "com.garmin.android.apps.connectmobile";

/// Browser-like user agent for SSO pages (avoids Cloudflare challenges)
const SSO_USER_AGENT: &str = "Mozilla/5.0 (iPhone; CPU iPhone OS 18_7 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Mobile/15E148";

/// Built-in OAuth consumer credentials (Garmin Connect Android app)
const OAUTH_CONSUMER_KEY: &str = "fc3e99d2-118c-44b8-8ae3-03370dde24c0";
const OAUTH_CONSUMER_SECRET: &str = "E08WAR897WEy2knn7aFBrvegVAf0AFdWBBF";

/// URL to fetch OAuth consumer credentials (fallback if the built-in ones are rejected)
const OAUTH_CONSUMER_URL: &str = "https://thegarth.s3.amazonaws.com/oauth_consumer.json";

/// OAuth consumer credentials fetched from Garth's S3
#[derive(Debug, Deserialize)]
struct OAuthConsumerResponse {
    consumer_key: String,
    consumer_secret: String,
}

fn builtin_oauth_consumer() -> OAuthConsumer {
    OAuthConsumer {
        key: OAUTH_CONSUMER_KEY.to_string(),
        secret: OAUTH_CONSUMER_SECRET.to_string(),
    }
}

/// SSO login request body
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct LoginRequest<'a> {
    username: &'a str,
    password: &'a str,
    remember_me: bool,
    captcha_token: &'a str,
}

/// SSO MFA verification request body
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct MfaVerifyRequest<'a> {
    mfa_method: &'a str,
    mfa_verification_code: &'a str,
    remember_my_browser: bool,
    reconsent_list: Vec<String>,
    mfa_setup: bool,
}

/// SSO response status
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SsoResponseStatus {
    #[serde(rename = "type")]
    response_type: String,
    #[serde(default)]
    message: String,
}

/// SSO login/MFA response
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SsoResponse {
    #[serde(default)]
    response_status: Option<SsoResponseStatus>,
    #[serde(default)]
    service_ticket_id: Option<String>,
    #[serde(default)]
    customer_mfa_info: Option<MfaInfo>,
}

/// MFA information from login response
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MfaInfo {
    #[serde(default)]
    mfa_last_method_used: Option<String>,
}

/// SSO Client for Garmin authentication
pub struct SsoClient {
    cookies: CookieStore,
    domain: String,
}

impl SsoClient {
    /// Create a new SSO client for the international domain (`garmin.com`).
    pub fn new() -> Result<Self> {
        Self::with_domain(GLOBAL_DOMAIN)
    }

    /// Create a new SSO client for a specific Garmin domain (`garmin.com`
    /// for international accounts, `garmin.cn` for mainland China accounts).
    pub fn with_domain(domain: impl Into<String>) -> Result<Self> {
        Ok(Self {
            cookies: CookieStore::default(),
            domain: domain.into(),
        })
    }

    /// The Garmin domain this client authenticates against.
    pub fn domain(&self) -> &str {
        &self.domain
    }

    /// Build SSO page headers (browser-like to establish session cookies)
    fn sso_page_headers(fetch_site: &'static str) -> Vec<Header<'static>> {
        vec![
            Header::UserAgent(SSO_USER_AGENT.into()),
            Header::Accept(
                "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8".into(),
            ),
            Header::AcceptLanguage("en-US,en;q=0.9".into()),
            Header::Custom("Sec-Fetch-Mode".into(), "navigate".into()),
            Header::Custom("Sec-Fetch-Dest".into(), "document".into()),
            Header::Custom("Sec-Fetch-Site".into(), fetch_site.into()),
        ]
    }

    /// Build mobile JSON API headers for login/MFA endpoints
    fn sso_api_headers(&self) -> Vec<Header<'static>> {
        vec![
            Header::UserAgent(MOBILE_USER_AGENT.into()),
            Header::Accept("application/json, text/plain, */*".into()),
            Header::AcceptLanguage("en-US,en;q=0.9".into()),
            Header::ContentType("application/json".into()),
            Header::Origin(format!("https://sso.{}", self.domain).into()),
            Header::Referer(format!("https://sso.{}/mobile/sso/en/sign-in", self.domain).into()),
        ]
    }

    /// Send `request` to an SSO page, carrying and updating the session cookies
    async fn send_with_cookies(&self, request: Client<'static>, url: String) -> Result<Response> {
        let request = match self.cookies.header() {
            Some(cookie) => request.header(cookie),
            None => request,
        };
        let response = http::send(request, url).await?;
        self.cookies.store(&response);
        Ok(response)
    }

    /// Query parameters for the login and MFA endpoints
    fn query(params: &[(&str, &str)]) -> Vec<curl_rest::QueryParam<'static>> {
        params
            .iter()
            .map(|(k, v)| curl_rest::QueryParam::new(k.to_string(), v.to_string()))
            .collect()
    }

    fn to_json<B: Serialize>(body: &B) -> Result<String> {
        serde_json::to_string(body)
            .map_err(|e| GarminError::config(format!("Failed to serialize request body: {}", e)))
    }

    /// Perform full login flow using mobile JSON API
    pub async fn login(
        &mut self,
        email: &str,
        password: &str,
        mfa_callback: Option<impl FnOnce() -> String>,
    ) -> Result<(OAuth1Token, OAuth2Token)> {
        let service_url = format!("https://mobile.integration.{}/gcm/android", self.domain);
        let login_params = [
            ("clientId", CLIENT_ID),
            ("locale", "en-US"),
            ("service", service_url.as_str()),
        ];

        // Step 1: Set cookies by visiting the sign-in page
        let sign_in_url = format!("https://sso.{}/mobile/sso/en/sign-in", self.domain);
        let request = Client::new()
            .get()
            .query_param_kv("clientId", CLIENT_ID)
            .headers(Self::sso_page_headers("none"));

        self.send_with_cookies(request, sign_in_url).await?;

        // Step 2: Submit login via JSON API
        let login_url = format!("https://sso.{}/mobile/api/login", self.domain);
        let login_body = LoginRequest {
            username: email,
            password,
            remember_me: false,
            captcha_token: "",
        };

        let request = Client::new()
            .post()
            .query_params(Self::query(&login_params))
            .headers(self.sso_api_headers())
            .body_json(Self::to_json(&login_body)?);
        let response = self.send_with_cookies(request, login_url).await?;

        let status_code = response.status;
        let body_text = http::body_text(&response);

        if status_code == StatusCode::TOO_MANY_REQUESTS {
            return Err(GarminError::auth(format!(
                "Rate limited by Garmin (429). Response body: {}",
                &body_text[..body_text.len().min(400)]
            )));
        }
        if !http::is_success(status_code) {
            return Err(GarminError::auth(format!(
                "SSO HTTP {}. Response body: {}",
                status_code,
                &body_text[..body_text.len().min(400)]
            )));
        }

        let sso_resp: SsoResponse = serde_json::from_str(&body_text).map_err(|e| {
            GarminError::invalid_response(format!(
                "Failed to parse SSO response: {} | body: {}",
                e,
                &body_text[..body_text.len().min(200)]
            ))
        })?;

        let resp_type = sso_resp
            .response_status
            .as_ref()
            .map(|s| s.response_type.as_str())
            .unwrap_or("UNKNOWN");

        let ticket = match resp_type {
            "SUCCESSFUL" => sso_resp
                .service_ticket_id
                .ok_or_else(|| GarminError::invalid_response("Missing serviceTicketId"))?,

            "MFA_REQUIRED" => {
                let mfa_method = sso_resp
                    .customer_mfa_info
                    .and_then(|info| info.mfa_last_method_used)
                    .unwrap_or_else(|| "email".to_string());

                let mfa_code = mfa_callback.ok_or_else(|| GarminError::MfaRequired)?();

                self.submit_mfa(&mfa_code, &mfa_method, &login_params)
                    .await?
            }

            _ => {
                let message = sso_resp
                    .response_status
                    .map(|s| {
                        if s.message.is_empty() {
                            s.response_type
                        } else {
                            format!("{}: {}", s.response_type, s.message)
                        }
                    })
                    .unwrap_or_else(|| "Unknown error".to_string());
                return Err(GarminError::auth(format!("SSO error: {}", message)));
            }
        };

        // Step 3: Complete login (set Cloudflare LB cookie, get OAuth tokens)
        self.complete_login(&ticket).await
    }

    /// Submit MFA verification code via JSON API
    async fn submit_mfa(
        &self,
        mfa_code: &str,
        mfa_method: &str,
        login_params: &[(&str, &str)],
    ) -> Result<String> {
        let mfa_url = format!("https://sso.{}/mobile/api/mfa/verifyCode", self.domain);
        let mfa_body = MfaVerifyRequest {
            mfa_method,
            mfa_verification_code: mfa_code,
            remember_my_browser: false,
            reconsent_list: vec![],
            mfa_setup: false,
        };

        let request = Client::new()
            .post()
            .query_params(Self::query(login_params))
            .headers(self.sso_api_headers())
            .body_json(Self::to_json(&mfa_body)?);
        let response = self.send_with_cookies(request, mfa_url).await?;

        let sso_resp: SsoResponse = serde_json::from_slice(&response.body).map_err(|e| {
            GarminError::invalid_response(format!("Failed to parse MFA response: {}", e))
        })?;

        let resp_type = sso_resp
            .response_status
            .as_ref()
            .map(|s| s.response_type.as_str())
            .unwrap_or("UNKNOWN");

        if resp_type != "SUCCESSFUL" {
            let message = sso_resp
                .response_status
                .map(|s| s.message)
                .unwrap_or_default();
            return Err(GarminError::auth(format!(
                "MFA verification failed: {}",
                message
            )));
        }

        sso_resp
            .service_ticket_id
            .ok_or_else(|| GarminError::invalid_response("Missing serviceTicketId after MFA"))
    }

    /// Complete login: set Cloudflare LB cookie and exchange for OAuth tokens
    async fn complete_login(&self, ticket: &str) -> Result<(OAuth1Token, OAuth2Token)> {
        // Best-effort: set Cloudflare LB cookie for backend pinning
        let portal_url = format!("https://sso.{}/portal/sso/embed", self.domain);
        let request = Client::new()
            .get()
            .headers(Self::sso_page_headers("same-origin"));
        let _ = self.send_with_cookies(request, portal_url).await;

        // Exchange ticket for OAuth1 token
        let oauth1 = self.get_oauth1_token(ticket).await?;

        // Exchange OAuth1 for OAuth2 token
        let oauth2 = self.exchange_oauth1_for_oauth2(&oauth1, true).await?;

        Ok((oauth1, oauth2))
    }

    /// Exchange ticket for OAuth1 token
    async fn get_oauth1_token(&self, ticket: &str) -> Result<OAuth1Token> {
        match self
            .get_oauth1_token_with(ticket, &builtin_oauth_consumer())
            .await
        {
            Ok(token) => Ok(token),
            Err(e) => match self.fallback_oauth_consumer().await {
                Some(consumer) => self.get_oauth1_token_with(ticket, &consumer).await,
                None => Err(e),
            },
        }
    }

    async fn get_oauth1_token_with(
        &self,
        ticket: &str,
        consumer: &OAuthConsumer,
    ) -> Result<OAuth1Token> {
        let base_url = format!("https://connectapi.{}/oauth-service/oauth/", self.domain);
        let login_url = format!("https://mobile.integration.{}/gcm/android", self.domain);
        let url = format!(
            "{}preauthorized?ticket={}&login-url={}&accepts-mfa-tokens=true",
            base_url, ticket, login_url
        );

        let signer = OAuth1Signer::new(consumer.clone());

        let auth_header = signer.sign("GET", &url, &[]);

        // Sent without cookies (matches garth's OAuth1Session behavior)
        let request = Client::new()
            .get()
            .header(Header::UserAgent(MOBILE_USER_AGENT.into()))
            .header(Header::Authorization(auth_header.into()));
        let response = http::send(request, url).await?;

        let status = response.status;
        if !http::is_success(status) {
            return Err(GarminError::auth(format!(
                "Failed to get OAuth1 token: {}",
                status
            )));
        }

        let params = parse_oauth_response(&http::body_text(&response));

        let oauth_token = params
            .get("oauth_token")
            .ok_or_else(|| GarminError::invalid_response("Missing oauth_token"))?
            .clone();
        let oauth_token_secret = params
            .get("oauth_token_secret")
            .ok_or_else(|| GarminError::invalid_response("Missing oauth_token_secret"))?
            .clone();
        let mfa_token = params.get("mfa_token").cloned();

        let mut token =
            OAuth1Token::new(oauth_token, oauth_token_secret).with_domain(self.domain.clone());

        if let Some(mfa) = mfa_token {
            token = token.with_mfa(mfa, None);
        }

        Ok(token)
    }

    /// Exchange OAuth1 token for OAuth2 token
    async fn exchange_oauth1_for_oauth2(
        &self,
        oauth1: &OAuth1Token,
        login: bool,
    ) -> Result<OAuth2Token> {
        match self
            .exchange_oauth1_for_oauth2_with(oauth1, login, &builtin_oauth_consumer())
            .await
        {
            Ok(token) => Ok(token),
            Err(e) => match self.fallback_oauth_consumer().await {
                Some(consumer) => {
                    self.exchange_oauth1_for_oauth2_with(oauth1, login, &consumer)
                        .await
                }
                None => Err(e),
            },
        }
    }

    async fn exchange_oauth1_for_oauth2_with(
        &self,
        oauth1: &OAuth1Token,
        login: bool,
        consumer: &OAuthConsumer,
    ) -> Result<OAuth2Token> {
        let url = format!(
            "https://connectapi.{}/oauth-service/oauth/exchange/user/2.0",
            self.domain
        );

        let signer = OAuth1Signer::new(consumer.clone()).with_token(OAuthToken {
            token: oauth1.oauth_token.clone(),
            secret: oauth1.oauth_token_secret.clone(),
        });

        let mut form_params: Vec<(String, String)> = vec![];
        if login {
            form_params.push((
                "audience".to_string(),
                "GARMIN_CONNECT_MOBILE_ANDROID_DI".to_string(),
            ));
        }
        if let Some(ref mfa_token) = oauth1.mfa_token {
            form_params.push(("mfa_token".to_string(), mfa_token.clone()));
        }

        let auth_header = signer.sign("POST", &url, &form_params);

        // Always send the (possibly empty) form so libcurl posts a zero-length body
        let form = url::form_urlencoded::Serializer::new(String::new())
            .extend_pairs(&form_params)
            .finish();

        let request = Client::new()
            .post()
            .header(Header::UserAgent(MOBILE_USER_AGENT.into()))
            .header(Header::Authorization(auth_header.into()))
            .header(Header::ContentType(
                "application/x-www-form-urlencoded".into(),
            ))
            .body_bytes(form.into_bytes());
        let response = http::send(request, url).await?;

        let status = response.status;
        if !http::is_success(status) {
            return Err(GarminError::auth(format!(
                "Failed to exchange OAuth1 for OAuth2: {}",
                status
            )));
        }

        let mut token: OAuth2Token = serde_json::from_slice(&response.body).map_err(|e| {
            GarminError::invalid_response(format!("Failed to parse OAuth2 token: {}", e))
        })?;

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        token.expires_at = now + token.expires_in;
        token.refresh_token_expires_at = now + token.refresh_token_expires_in;

        Ok(token)
    }

    /// Consumer credentials from Garth's S3, only if they differ from the built-in ones
    async fn fallback_oauth_consumer(&self) -> Option<OAuthConsumer> {
        let remote = self.fetch_oauth_consumer().await.ok()?;
        if remote.consumer_key == OAUTH_CONSUMER_KEY
            && remote.consumer_secret == OAUTH_CONSUMER_SECRET
        {
            return None;
        }
        Some(OAuthConsumer {
            key: remote.consumer_key,
            secret: remote.consumer_secret,
        })
    }

    /// Fetch OAuth consumer credentials from Garth's S3
    async fn fetch_oauth_consumer(&self) -> Result<OAuthConsumerResponse> {
        let response = http::send(Client::new().get(), OAUTH_CONSUMER_URL.to_string()).await?;

        serde_json::from_slice(&response.body).map_err(|e| {
            GarminError::invalid_response(format!("Failed to parse OAuth consumer: {}", e))
        })
    }

    /// Refresh OAuth2 token using OAuth1 token
    pub async fn refresh_oauth2(&self, oauth1: &OAuth1Token) -> Result<OAuth2Token> {
        self.exchange_oauth1_for_oauth2(oauth1, false).await
    }
}

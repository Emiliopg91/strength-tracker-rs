use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::GLOBAL_DOMAIN;

/// OAuth1 token obtained after initial SSO authentication.
/// Long-lived (~1 year), used to obtain OAuth2 tokens.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OAuth1Token {
    pub oauth_token: String,
    pub oauth_token_secret: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mfa_token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mfa_expiration_timestamp: Option<DateTime<Utc>>,
    #[serde(default = "default_domain")]
    pub domain: String,
}

fn default_domain() -> String {
    GLOBAL_DOMAIN.to_string()
}

impl OAuth1Token {
    pub fn new(oauth_token: String, oauth_token_secret: String) -> Self {
        Self {
            oauth_token,
            oauth_token_secret,
            mfa_token: None,
            mfa_expiration_timestamp: None,
            domain: default_domain(),
        }
    }

    pub fn with_mfa(mut self, mfa_token: String, expiration: Option<DateTime<Utc>>) -> Self {
        self.mfa_token = Some(mfa_token);
        self.mfa_expiration_timestamp = expiration;
        self
    }

    /// Record which Garmin domain (region) this token was issued for.
    pub fn with_domain(mut self, domain: String) -> Self {
        self.domain = domain;
        self
    }
}

/// OAuth2 Bearer token for API requests.
/// Short-lived, refreshed using the OAuth1 token.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OAuth2Token {
    pub scope: String,
    pub jti: String,
    pub token_type: String,
    pub access_token: String,
    pub refresh_token: String,
    pub expires_in: i64,
    #[serde(default)]
    pub expires_at: i64,
    pub refresh_token_expires_in: i64,
    #[serde(default)]
    pub refresh_token_expires_at: i64,
}

impl OAuth2Token {
    pub fn is_expired(&self) -> bool {
        self.expires_at < Utc::now().timestamp()
    }

    pub fn is_refresh_expired(&self) -> bool {
        self.refresh_token_expires_at < Utc::now().timestamp()
    }

    pub fn authorization_header(&self) -> String {
        format!("{} {}", self.token_type, self.access_token)
    }
}

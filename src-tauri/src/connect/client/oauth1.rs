//! OAuth1 request signing (HMAC-SHA1) as required by Garmin's OAuth service.

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use hmac::{Hmac, KeyInit, Mac};
use percent_encoding::{AsciiSet, CONTROLS, utf8_percent_encode};
use sha1::Sha1;
use std::collections::BTreeMap;
use std::time::{SystemTime, UNIX_EPOCH};
use url::Url;

/// Characters that need to be percent-encoded in OAuth1
const ENCODE_SET: &AsciiSet = &CONTROLS
    .add(b' ')
    .add(b'"')
    .add(b'#')
    .add(b'$')
    .add(b'%')
    .add(b'&')
    .add(b'+')
    .add(b',')
    .add(b'/')
    .add(b':')
    .add(b';')
    .add(b'<')
    .add(b'=')
    .add(b'>')
    .add(b'?')
    .add(b'@')
    .add(b'[')
    .add(b'\\')
    .add(b']')
    .add(b'^')
    .add(b'`')
    .add(b'{')
    .add(b'|')
    .add(b'}');

#[derive(Debug, Clone)]
pub struct OAuthConsumer {
    pub key: String,
    pub secret: String,
}

#[derive(Debug, Clone, Default)]
pub struct OAuthToken {
    pub token: String,
    pub secret: String,
}

pub struct OAuth1Signer {
    consumer: OAuthConsumer,
    token: Option<OAuthToken>,
}

impl OAuth1Signer {
    pub fn new(consumer: OAuthConsumer) -> Self {
        Self {
            consumer,
            token: None,
        }
    }

    pub fn with_token(mut self, token: OAuthToken) -> Self {
        self.token = Some(token);
        self
    }

    /// Generate the OAuth1 `Authorization` header for a request.
    ///
    /// Query parameters in `url` are extracted and included in the signature.
    pub fn sign(&self, method: &str, url: &str, extra_params: &[(String, String)]) -> String {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
            .to_string();

        let nonce: String = rand::random::<[u8; 16]>()
            .iter()
            .map(|b| format!("{:02x}", b))
            .collect();

        let parsed_url = Url::parse(url).expect("Invalid URL");
        let base_url = format!(
            "{}://{}{}",
            parsed_url.scheme(),
            parsed_url.host_str().unwrap_or(""),
            parsed_url.path()
        );
        let url_params: Vec<(String, String)> = parsed_url
            .query_pairs()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();

        let mut oauth_params: BTreeMap<String, String> = BTreeMap::new();
        oauth_params.insert("oauth_consumer_key".to_string(), self.consumer.key.clone());
        oauth_params.insert("oauth_nonce".to_string(), nonce);
        oauth_params.insert(
            "oauth_signature_method".to_string(),
            "HMAC-SHA1".to_string(),
        );
        oauth_params.insert("oauth_timestamp".to_string(), timestamp);
        oauth_params.insert("oauth_version".to_string(), "1.0".to_string());

        if let Some(token) = &self.token {
            oauth_params.insert("oauth_token".to_string(), token.token.clone());
        }

        let signature =
            self.calculate_signature(method, &base_url, &url_params, extra_params, &oauth_params);
        oauth_params.insert("oauth_signature".to_string(), signature);

        let auth_params: Vec<String> = oauth_params
            .iter()
            .map(|(k, v)| format!("{}=\"{}\"", k, percent_encode(v)))
            .collect();

        format!("OAuth {}", auth_params.join(", "))
    }

    fn calculate_signature(
        &self,
        method: &str,
        base_url: &str,
        url_params: &[(String, String)],
        extra_params: &[(String, String)],
        oauth_params: &BTreeMap<String, String>,
    ) -> String {
        let mut all_params = oauth_params.clone();
        for (k, v) in url_params.iter().chain(extra_params) {
            all_params.insert(k.clone(), v.clone());
        }

        let param_string = all_params
            .iter()
            .map(|(k, v)| format!("{}={}", percent_encode(k), percent_encode(v)))
            .collect::<Vec<_>>()
            .join("&");

        let base_string = format!(
            "{}&{}&{}",
            method.to_uppercase(),
            percent_encode(base_url),
            percent_encode(&param_string)
        );

        let token_secret = self.token.as_ref().map(|t| t.secret.as_str()).unwrap_or("");
        let signing_key = format!(
            "{}&{}",
            percent_encode(&self.consumer.secret),
            percent_encode(token_secret)
        );

        let mut mac = Hmac::<Sha1>::new_from_slice(signing_key.as_bytes())
            .expect("HMAC can take key of any size");
        mac.update(base_string.as_bytes());

        STANDARD.encode(mac.finalize().into_bytes())
    }
}

fn percent_encode(s: &str) -> String {
    utf8_percent_encode(s, ENCODE_SET).to_string()
}

/// Parse an OAuth1 response body (URL-encoded key=value pairs)
pub fn parse_oauth_response(response: &str) -> BTreeMap<String, String> {
    url::form_urlencoded::parse(response.as_bytes())
        .into_owned()
        .collect()
}

//! Authenticated Garmin Connect API client (OAuth2 bearer tokens).

use bytes::Bytes;
use reqwest::header::{AUTHORIZATION, HeaderMap, HeaderValue, USER_AGENT};
use reqwest::{Client, Response, StatusCode};
use serde::de::DeserializeOwned;

use super::error::{GarminError, Result};
use super::models::{ActivityDetails, ActivitySummary};
use super::tokens::OAuth2Token;

/// User agent for Connect API requests
const API_USER_AGENT: &str = "GCM-iOS-5.7.2.1";

#[derive(Clone)]
pub struct GarminClient {
    client: Client,
    base_url: String,
}

impl GarminClient {
    /// Create an API client for the given Garmin domain (`garmin.com` or `garmin.cn`).
    pub fn new(domain: &str) -> Result<Self> {
        Ok(Self {
            client: Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()?,
            base_url: format!("https://connectapi.{}", domain),
        })
    }

    /// List activities, most recent first.
    pub async fn list_activities(
        &self,
        token: &OAuth2Token,
        limit: u32,
        start: u32,
    ) -> Result<Vec<ActivitySummary>> {
        let path = format!(
            "/activitylist-service/activities/search/activities?limit={}&start={}",
            limit, start
        );
        self.get_json(token, &path).await
    }

    pub async fn get_activity(&self, token: &OAuth2Token, id: u64) -> Result<ActivityDetails> {
        self.get_json(token, &format!("/activity-service/activity/{}", id))
            .await
    }

    /// Make an authenticated GET request and deserialize the JSON response
    pub async fn get_json<T: DeserializeOwned>(
        &self,
        token: &OAuth2Token,
        path: &str,
    ) -> Result<T> {
        let response = self
            .client
            .get(format!("{}{}", self.base_url, path))
            .headers(Self::build_headers(token)?)
            .send()
            .await?;

        let response = Self::handle_response_status(response).await?;
        let status = response.status();
        let text = response.text().await?;

        serde_json::from_str(&text).map_err(|e| {
            GarminError::invalid_response(format!(
                "Failed to parse JSON response from {} (status {}): {}; body preview: {}",
                path,
                status,
                e,
                response_preview(&text)
            ))
        })
    }

    /// Make an authenticated GET request and return raw bytes (for file downloads)
    pub async fn download(&self, token: &OAuth2Token, path: &str) -> Result<Bytes> {
        let response = self
            .client
            .get(format!("{}{}", self.base_url, path))
            .headers(Self::build_headers(token)?)
            .send()
            .await?;

        let response = Self::handle_response_status(response).await?;
        Ok(response.bytes().await?)
    }

    fn build_headers(token: &OAuth2Token) -> Result<HeaderMap> {
        let mut headers = HeaderMap::new();
        headers.insert(USER_AGENT, HeaderValue::from_static(API_USER_AGENT));
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&token.authorization_header())
                .map_err(|_| GarminError::config("Invalid authorization header"))?,
        );
        Ok(headers)
    }

    async fn handle_response_status(response: Response) -> Result<Response> {
        let status = response.status();

        match status {
            StatusCode::OK
            | StatusCode::CREATED
            | StatusCode::ACCEPTED
            | StatusCode::NO_CONTENT => Ok(response),
            StatusCode::UNAUTHORIZED => Err(GarminError::NotAuthenticated),
            StatusCode::TOO_MANY_REQUESTS => Err(GarminError::RateLimited),
            StatusCode::NOT_FOUND => Err(GarminError::NotFound(response.url().to_string())),
            _ => Err(GarminError::Api {
                status: status.as_u16(),
                message: response.text().await.unwrap_or_default(),
            }),
        }
    }
}

fn response_preview(text: &str) -> String {
    const MAX_PREVIEW_CHARS: usize = 160;
    let compact = text.split_whitespace().collect::<Vec<_>>().join(" ");
    let preview = compact.chars().take(MAX_PREVIEW_CHARS).collect::<String>();
    if compact.chars().count() > MAX_PREVIEW_CHARS {
        format!("{}...", preview)
    } else if preview.is_empty() {
        "<empty>".to_string()
    } else {
        preview
    }
}

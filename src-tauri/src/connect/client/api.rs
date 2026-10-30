//! Authenticated Garmin Connect API client (OAuth2 bearer tokens).

use bytes::Bytes;
use curl_rest::{Client, Header, Response, StatusCode};
use serde::{Serialize, de::DeserializeOwned};

use super::error::{GarminError, Result};
use super::http;
use super::models::{ActivityDetails, ActivitySummary};
use super::tokens::OAuth2Token;

/// User agent for Connect API requests
const API_USER_AGENT: &str = "GCM-iOS-5.7.2.1";

#[derive(Clone)]
pub struct GarminClient {
    base_url: String,
}

impl GarminClient {
    /// Create an API client for the given Garmin domain (`garmin.com` or `garmin.cn`).
    pub fn new(domain: &str) -> Result<Self> {
        Ok(Self {
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
        let response = self.send(Self::request(token).get(), path).await?;
        Self::parse_json(path, &response)
    }

    /// Make an authenticated POST request with a JSON body and deserialize the JSON response
    pub async fn post_json<B: Serialize + ?Sized, T: DeserializeOwned>(
        &self,
        token: &OAuth2Token,
        path: &str,
        body: &B,
    ) -> Result<T> {
        let request = Self::request(token).post().body_json(Self::to_json(body)?);
        let response = self.send(request, path).await?;
        Self::parse_json(path, &response)
    }

    /// Make an authenticated PUT request with a JSON body, discarding the response body
    pub async fn put_json<B: Serialize + ?Sized>(
        &self,
        token: &OAuth2Token,
        path: &str,
        body: &B,
    ) -> Result<()> {
        let request = Self::request(token).put().body_json(Self::to_json(body)?);
        self.send(request, path).await?;
        Ok(())
    }

    /// Make an authenticated DELETE request, discarding the response body
    pub async fn delete(&self, token: &OAuth2Token, path: &str) -> Result<()> {
        self.send(Self::request(token).delete(), path).await?;
        Ok(())
    }

    /// Make an authenticated GET request and return raw bytes (for file downloads)
    pub async fn download(&self, token: &OAuth2Token, path: &str) -> Result<Bytes> {
        let response = self.send(Self::request(token).get(), path).await?;
        Ok(Bytes::from(response.body))
    }

    fn request(token: &OAuth2Token) -> Client<'static> {
        Client::new()
            .header(Header::UserAgent(API_USER_AGENT.into()))
            .header(Header::Authorization(token.authorization_header().into()))
    }

    fn to_json<B: Serialize + ?Sized>(body: &B) -> Result<String> {
        serde_json::to_string(body)
            .map_err(|e| GarminError::config(format!("Failed to serialize request body: {}", e)))
    }

    fn parse_json<T: DeserializeOwned>(path: &str, response: &Response) -> Result<T> {
        serde_json::from_slice(&response.body).map_err(|e| {
            GarminError::invalid_response(format!(
                "Failed to parse JSON response from {} (status {}): {}; body preview: {}",
                path,
                response.status,
                e,
                response_preview(&http::body_text(response))
            ))
        })
    }

    async fn send(&self, request: Client<'static>, path: &str) -> Result<Response> {
        let url = format!("{}{}", self.base_url, path);
        let response = http::send(request, url.clone()).await?;

        match response.status {
            StatusCode::OK
            | StatusCode::CREATED
            | StatusCode::ACCEPTED
            | StatusCode::NO_CONTENT => Ok(response),
            StatusCode::UNAUTHORIZED => Err(GarminError::NotAuthenticated),
            StatusCode::TOO_MANY_REQUESTS => Err(GarminError::RateLimited),
            StatusCode::NOT_FOUND => Err(GarminError::NotFound(url)),
            status => Err(GarminError::Api {
                status: status.as_u16(),
                message: http::body_text(&response),
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

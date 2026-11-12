//! Authenticated Garmin Connect API client (OAuth2 bearer tokens).

use bytes::Bytes;
use reqwest::header::{AUTHORIZATION, USER_AGENT};
use reqwest::{Method, RequestBuilder, StatusCode};
use serde::{Serialize, de::DeserializeOwned};

use super::error::{GarminError, Result};
use super::http::{self, Response};
use super::models::{
    ActivityDescriptionUpdate, ActivityDetails, ActivitySummary, CalendarItem, CalendarMonth,
    DeviceSummary, ExerciseSets,
};
use super::tokens::OAuth2Token;

/// User agent for Connect API requests
const API_USER_AGENT: &str = "GCM-iOS-5.7.2.1";

#[derive(Clone)]
pub struct GarminClient {
    base_url: String,
}

/// Path of the calendar page of `month` (1-12). The calendar service numbers months from 0.
pub fn calendar_month_path(year: u16, month: u8) -> Result<String> {
    if !(1..=12).contains(&month) {
        return Err(GarminError::config(format!("Invalid month: {month}")));
    }
    Ok(format!(
        "/calendar-service/year/{}/month/{}",
        year,
        month - 1
    ))
}

fn exercise_sets_path(id: u64) -> String {
    format!("/activity-service/activity/{}/exerciseSets", id)
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

    /// Replace the description (notes) of activity `id`, leaving the rest of it untouched.
    pub async fn update_activity_description(
        &self,
        token: &OAuth2Token,
        id: u64,
        description: &str,
    ) -> Result<()> {
        let body = ActivityDescriptionUpdate {
            activity_id: id,
            description,
        };
        self.put_json(token, &format!("/activity-service/activity/{}", id), &body)
            .await
    }

    /// Get the sets of the strength activity `id`.
    pub async fn get_exercise_sets(&self, token: &OAuth2Token, id: u64) -> Result<ExerciseSets> {
        self.get_json(token, &exercise_sets_path(id)).await
    }

    /// Replace every set of the strength activity `sets.activity_id` with `sets`.
    pub async fn update_exercise_sets(
        &self,
        token: &OAuth2Token,
        sets: &ExerciseSets,
    ) -> Result<()> {
        self.put_json(token, &exercise_sets_path(sets.activity_id), sets)
            .await
    }

    /// List every device registered to the account.
    pub async fn list_devices(&self, token: &OAuth2Token) -> Result<Vec<DeviceSummary>> {
        self.get_json(token, "/device-service/deviceregistration/devices")
            .await
    }

    /// List the items of the calendar page of `month` (1-12), which includes days of the adjacent
    /// months.
    pub async fn list_calendar_month(
        &self,
        token: &OAuth2Token,
        year: u16,
        month: u8,
    ) -> Result<Vec<CalendarItem>> {
        let path = calendar_month_path(year, month)?;
        let calendar: CalendarMonth = self.get_json(token, &path).await?;
        Ok(calendar.calendar_items)
    }

    /// Make an authenticated GET request and deserialize the JSON response
    pub async fn get_json<T: DeserializeOwned>(
        &self,
        token: &OAuth2Token,
        path: &str,
    ) -> Result<T> {
        let response = self.send(token, Method::GET, path, |r| r).await?;
        Self::parse_json(path, &response)
    }

    /// Make an authenticated POST request with a JSON body and deserialize the JSON response
    pub async fn post_json<B: Serialize + ?Sized, T: DeserializeOwned>(
        &self,
        token: &OAuth2Token,
        path: &str,
        body: &B,
    ) -> Result<T> {
        let response = self
            .send(token, Method::POST, path, |r| r.json(body))
            .await?;
        Self::parse_json(path, &response)
    }

    /// Make an authenticated PUT request with a JSON body, discarding the response body
    pub async fn put_json<B: Serialize + ?Sized>(
        &self,
        token: &OAuth2Token,
        path: &str,
        body: &B,
    ) -> Result<()> {
        self.send(token, Method::PUT, path, |r| r.json(body))
            .await?;
        Ok(())
    }

    /// Make an authenticated DELETE request, discarding the response body
    pub async fn delete(&self, token: &OAuth2Token, path: &str) -> Result<()> {
        self.send(token, Method::DELETE, path, |r| r).await?;
        Ok(())
    }

    /// Make an authenticated GET request and return raw bytes (for file downloads)
    pub async fn download(&self, token: &OAuth2Token, path: &str) -> Result<Bytes> {
        let response = self.send(token, Method::GET, path, |r| r).await?;
        Ok(response.body)
    }

    fn parse_json<T: DeserializeOwned>(path: &str, response: &Response) -> Result<T> {
        serde_json::from_slice(&response.body).map_err(|e| {
            GarminError::invalid_response(format!(
                "Failed to parse JSON response from {} (status {}): {}; body preview: {}",
                path,
                response.status,
                e,
                response_preview(&response.text())
            ))
        })
    }

    /// Send an authenticated request to `path`, after `build` adds the body (if any)
    async fn send(
        &self,
        token: &OAuth2Token,
        method: Method,
        path: &str,
        build: impl FnOnce(RequestBuilder) -> RequestBuilder,
    ) -> Result<Response> {
        let url = format!("{}{}", self.base_url, path);
        let request = http::client()
            .request(method, &url)
            .header(USER_AGENT, API_USER_AGENT)
            .header(AUTHORIZATION, token.authorization_header());
        let response = http::send(build(request)).await?;

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
                message: response.text(),
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

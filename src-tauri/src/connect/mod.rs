#[allow(dead_code, unused_imports)]
pub mod client;
pub mod errors;

use crate::{
    connect::{
        client::{
            ActivitySummary, GarminClient, GarminError, OAuth1Token, OAuth2Token, SsoClient,
            WeightEntry, WeightRange,
        },
        errors::{ConnectError, Result},
    },
    dao::body_metric::BodyMetric,
    utils::constants,
};
use bytes::Bytes;
use futures_util::stream::{self, TryStreamExt};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use std::{
    collections::BTreeSet,
    io::{Cursor, Read},
    path::Path,
    time::Duration,
};
use tauri::{AppHandle, Emitter};

use tauri_plugin_log::log::{error, info, warn};

const KEYRING_EMAIL: &str = "garmin-email";
const KEYRING_OAUTH1: &str = "garmin-oauth1";
const KEYRING_OAUTH2: &str = "garmin-oauth2";

const DOWNLOAD_STEP: u8 = 100;
const DOWNLOAD_CONCURRENCY: usize = 6;
const WORKOUT_LIST_STEP: u8 = 100;
const WEIGHT_CHUNK_MONTHS: u32 = 12;
const WEIGHT_EMPTY_CHUNKS_STOP: u32 = 2;
const RETRY_ATTEMPTS: u32 = 4;
const RETRY_BASE_DELAY: Duration = Duration::from_secs(2);
const TOKEN_EXPIRY_MARGIN_SECS: i64 = 300;
/// Max gap between the end of a listed activity and the date of a stored session to match them.
const SKIP_TOLERANCE_SECS: u32 = 60;

/// Workout entry returned from the workout list and create endpoints
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkoutSummary {
    pub workout_id: u64,
    pub workout_name: String,
}

pub struct ConnectClient {
    app: AppHandle,
    email: Option<String>,
    oauth1: Option<OAuth1Token>,
    oauth2: Option<OAuth2Token>,
}

impl ConnectClient {
    pub fn new(app: AppHandle) -> Self {
        Self {
            app,
            email: Self::read_keyring(KEYRING_EMAIL).unwrap_or(None),
            oauth1: Self::read_keyring(KEYRING_OAUTH1).unwrap_or(None),
            oauth2: Self::read_keyring(KEYRING_OAUTH2).unwrap_or(None),
        }
    }

    pub fn logged_user(&self) -> Option<String> {
        self.email.clone()
    }

    pub fn disconnect(&mut self) -> Result<()> {
        Self::delete_keyring(KEYRING_OAUTH2)?;
        self.oauth2 = None;
        Self::delete_keyring(KEYRING_OAUTH1)?;
        self.oauth1 = None;
        Self::delete_keyring(KEYRING_EMAIL)?;
        self.email = None;

        let _ = self.app.emit("connect_logged", None::<String>);

        Ok(())
    }

    pub async fn login(&mut self, email: &str, password: &str) -> Result<()> {
        info!("Logging in Garmin Connect for user {}...", email);
        let mut sso = SsoClient::new()
            .map_err(|e| log_error("Error creating SSO client", ConnectError::Login(e)))?;

        let (oauth1, oauth2) = sso
            .login(email, password, None::<fn() -> String>)
            .await
            .map_err(|e| log_error("Error logging in", ConnectError::Login(e)))?;

        Self::write_keyring(KEYRING_EMAIL, &email.to_string())?;
        Self::write_keyring(KEYRING_OAUTH1, &oauth1)?;
        Self::write_keyring(KEYRING_OAUTH2, &oauth2)?;
        self.email = Some(email.to_string());
        self.oauth1 = Some(oauth1);
        self.oauth2 = Some(oauth2);

        info!("Logged in succesfully");
        let email: Option<String> = Some(email.to_string());
        let app: AppHandle = self.app.clone();
        let _ = app.emit("connect_logged", email);

        Ok(())
    }

    pub async fn prompt_login(&mut self) -> Result<()> {
        let mut text = String::from("Sign in to Garmin Connect");
        loop {
            let output = tokio::process::Command::new("zenity")
                .args([
                    "--forms",
                    "--title=Garmin Connect",
                    &format!("--text={}", text),
                    "--add-entry=Email",
                    "--add-password=Password",
                    "--separator=\n",
                ])
                .output()
                .await
                .map_err(|e| {
                    log_error("Error launching zenity", ConnectError::CredentialsPrompt(e))
                })?;

            // Cancelled by the user
            if !output.status.success() {
                warn!("Garmin Connect login cancelled by the user");
                return Err(ConnectError::LoginCancelledByUser());
            }

            let stdout = String::from_utf8_lossy(&output.stdout);
            let stdout = stdout.strip_suffix('\n').unwrap_or(&stdout);
            let (email, password) = stdout.split_once('\n').unwrap_or((stdout, ""));

            match self.login(email, password).await {
                Ok(()) => return Ok(()),
                // Shown on the next prompt, escaped as zenity parses the text as Pango markup
                Err(e) => {
                    text = format!(
                        "Sign in to Garmin Connect\n\nLogin failed: {}",
                        e.to_string()
                            .replace('&', "&amp;")
                            .replace('<', "&lt;")
                            .replace('>', "&gt;")
                    )
                }
            }
        }
    }

    pub async fn refresh_token(&mut self) -> Result<()> {
        // Refresh ahead of the real expiration so the token does not run out mid-download
        if let Some(oauth2) = &self.oauth2
            && oauth2.expires_at - TOKEN_EXPIRY_MARGIN_SECS > chrono::Utc::now().timestamp()
        {
            return Ok(());
        }

        info!("Refreshing OAuth2 token...");
        if let Some(oauth1) = &self.oauth1 {
            let sso = SsoClient::with_domain(oauth1.domain.clone()).map_err(|e| {
                log_error("Error creating SSO client", ConnectError::OAuthRefresh(e))
            })?;
            match with_retry(|| sso.refresh_oauth2(oauth1)).await {
                Ok(oauth2) => {
                    Self::write_keyring(KEYRING_OAUTH2, &oauth2)?;
                    self.oauth2 = Some(oauth2);

                    info!("Token refreshed successfully");

                    return Ok(());
                }
                Err(e @ (GarminError::Authentication(_) | GarminError::NotAuthenticated)) => {
                    // OAuth1 token likely expired or revoked: ask for a new login, which
                    // overwrites the stored tokens on success and keeps them if cancelled
                    warn!("Stored Garmin session was rejected ({}), login required", e);
                }
                Err(e) => {
                    return Err(log_error(
                        "Error refreshing OAuth2 token",
                        ConnectError::OAuthRefresh(e),
                    ));
                }
            }
        }

        match self.prompt_login().await {
            Ok(_) => Ok(()),
            Err(_) => Err(ConnectError::LoginRequired()),
        }
    }

    /// Downloads the activities started since `from`, except those already stored as a session in
    /// `skip`. Sessions are dated at their end, so an activity matches the session dated at its
    /// start plus its elapsed duration, give or take `SKIP_TOLERANCE_SECS`.
    /// Returns the start of the newest activity listed, skipped ones included.
    pub async fn download_activities<P>(
        &mut self,
        from: u32,
        skip: &BTreeSet<u32>,
        dst_path: P,
    ) -> Result<Option<u32>>
    where
        P: AsRef<Path>,
    {
        info!("Listing activities...");

        self.refresh_token().await?;

        let (Some(oauth1), Some(oauth2)) = (&self.oauth1, &self.oauth2) else {
            return Err(ConnectError::LoginRequired());
        };

        let client = GarminClient::new(&oauth1.domain).map_err(|e| {
            log_error(
                "Error creating Garmin client",
                ConnectError::FetchActivities(e),
            )
        })?;
        let start_date = chrono::DateTime::from_timestamp(from as i64, 0)
            .ok_or_else(|| {
                log_error(
                    "Error parsing start timestamp",
                    ConnectError::InvalidTimestamp(from),
                )
            })?
            .with_timezone(&chrono::Local)
            .format("%Y-%m-%d");

        let mut activities: Vec<(ActivitySummary, Option<u32>, Option<u32>)> = Vec::new();
        'pages: loop {
            let path = format!(
                "/activitylist-service/activities/search/activities?limit={}&start={}&startDate={}",
                DOWNLOAD_STEP,
                activities.len(),
                start_date
            );
            let result: Vec<ActivitySummary> = with_retry(|| client.get_json(oauth2, &path))
                .await
                .map_err(|e| {
                    log_error(
                        "Error fetching activity page",
                        ConnectError::FetchActivities(e),
                    )
                })?;
            let page_len = result.len();

            // Results come newest first: the first one older than `from` ends the listing
            for activity in result {
                // GMT, as the local start is in the timezone where the activity was recorded
                let start = activity
                    .start_time_gmt
                    .as_deref()
                    .and_then(|t| {
                        chrono::NaiveDateTime::parse_from_str(t, "%Y-%m-%d %H:%M:%S").ok()
                    })
                    .map(|t| t.and_utc().timestamp() as u32);
                if let Some(start) = start
                    && start < from
                {
                    break 'pages;
                }
                let end = start
                    .zip(activity.elapsed_duration.or(activity.duration))
                    .map(|(start, duration)| start + duration.round() as u32);
                activities.push((activity, start, end));
            }

            if page_len < DOWNLOAD_STEP as usize {
                break;
            }
        }
        info!("Listed {} activities.", activities.len());

        let newest = activities.iter().filter_map(|(_, start, _)| *start).max();
        let activities: Vec<ActivitySummary> = activities
            .into_iter()
            .filter(|(_, _, end)| {
                !end.is_some_and(|end| {
                    skip.range(end.saturating_sub(SKIP_TOLERANCE_SECS)..=end + SKIP_TOLERANCE_SECS)
                        .next()
                        .is_some()
                })
            })
            .map(|(activity, _, _)| activity)
            .collect();

        if !activities.is_empty() {
            info!("Downloading {} activities...", activities.len());
            let dst_path = dst_path.as_ref();
            stream::iter(activities.iter().map(Ok))
                .try_for_each_concurrent(DOWNLOAD_CONCURRENCY, |activity| {
                    Self::download_activity(&client, oauth2, activity.activity_id, dst_path)
                })
                .await?;
            info!("Downloaded {} activities.", activities.len());
        } else {
            info!("Activities up to date");
        }

        Ok(newest)
    }

    /// Lists every workout stored in Connect.
    pub async fn list_workouts(&mut self) -> Result<Vec<WorkoutSummary>> {
        info!("Listing workouts...");

        self.refresh_token().await?;

        let (Some(oauth1), Some(oauth2)) = (&self.oauth1, &self.oauth2) else {
            return Err(ConnectError::LoginRequired());
        };

        let client = GarminClient::new(&oauth1.domain).map_err(|e| {
            log_error(
                "Error creating Garmin client",
                ConnectError::FetchWorkouts(e),
            )
        })?;

        let workouts = Self::fetch_workouts(&client, oauth2).await?;
        info!("Listed {} workouts.", workouts.len());

        Ok(workouts)
    }

    /// Creates `workout` in Connect, returning the entry assigned to it.
    pub async fn create_workout<T: Serialize>(&mut self, workout: &T) -> Result<WorkoutSummary> {
        info!("Creating workout...");

        self.refresh_token().await?;

        let (Some(oauth1), Some(oauth2)) = (&self.oauth1, &self.oauth2) else {
            return Err(ConnectError::LoginRequired());
        };

        let client = GarminClient::new(&oauth1.domain).map_err(|e| {
            log_error(
                "Error creating Garmin client",
                ConnectError::CreateWorkout(e),
            )
        })?;

        // Not retried: a request that timed out after reaching Connect would duplicate the workout
        let created: WorkoutSummary = client
            .post_json(oauth2, "/workout-service/workout", workout)
            .await
            .map_err(|e| log_error("Error creating workout", ConnectError::CreateWorkout(e)))?;
        info!(
            "Created workout '{}' with id {}.",
            created.workout_name, created.workout_id
        );

        Ok(created)
    }

    /// Replaces the Connect workout `id` with `workout`, whose `workoutId` is set to `id`.
    pub async fn update_workout<T: Serialize>(&mut self, id: u64, workout: &T) -> Result<()> {
        info!("Updating workout {}...", id);

        let mut body = serde_json::to_value(workout)?;
        if let Some(fields) = body.as_object_mut() {
            fields.insert("workoutId".to_string(), id.into());
        }

        self.refresh_token().await?;

        let (Some(oauth1), Some(oauth2)) = (&self.oauth1, &self.oauth2) else {
            return Err(ConnectError::LoginRequired());
        };

        let client = GarminClient::new(&oauth1.domain).map_err(|e| {
            log_error(
                "Error creating Garmin client",
                ConnectError::UpdateWorkout(id, e),
            )
        })?;

        let path = format!("/workout-service/workout/{}", id);
        with_retry(|| client.put_json(oauth2, &path, &body))
            .await
            .map_err(|e| log_error("Error updating workout", ConnectError::UpdateWorkout(id, e)))?;
        info!("Updated workout {}.", id);

        Ok(())
    }

    pub async fn list_weight(&mut self, from: Option<u32>) -> Result<Vec<BodyMetric>> {
        info!("Listing weight entries...");

        self.refresh_token().await?;

        let (Some(oauth1), Some(oauth2)) = (&self.oauth1, &self.oauth2) else {
            return Err(ConnectError::LoginRequired());
        };

        let client = GarminClient::new(&oauth1.domain)
            .map_err(|e| log_error("Error creating Garmin client", ConnectError::FetchWeight(e)))?;
        let end_date = chrono::Local::now().date_naive();

        let entries = match from {
            Some(from) => {
                let start_date = chrono::DateTime::from_timestamp(from as i64, 0)
                    .ok_or_else(|| {
                        log_error(
                            "Error parsing start timestamp",
                            ConnectError::InvalidTimestamp(from),
                        )
                    })?
                    .with_timezone(&chrono::Local)
                    .date_naive();
                Self::fetch_weight_range(&client, oauth2, start_date, end_date).await?
            }
            None => {
                // No start date: walk back year by year until history runs dry
                let mut entries = Vec::new();
                let mut chunk_end = end_date;
                let mut empty_chunks = 0;
                while empty_chunks < WEIGHT_EMPTY_CHUNKS_STOP {
                    let chunk_start = chunk_end
                        .checked_sub_months(chrono::Months::new(WEIGHT_CHUNK_MONTHS))
                        .and_then(|d| d.succ_opt())
                        .unwrap_or(chrono::NaiveDate::MIN);
                    let chunk =
                        Self::fetch_weight_range(&client, oauth2, chunk_start, chunk_end).await?;
                    if chunk.is_empty() {
                        empty_chunks += 1;
                    } else {
                        empty_chunks = 0;
                        entries.extend(chunk);
                    }
                    let Some(prev) = chunk_start.pred_opt() else {
                        break;
                    };
                    chunk_end = prev;
                }
                entries
            }
        };

        let metrics: Vec<BodyMetric> = entries
            .iter()
            .filter_map(|w| BodyMetric::try_from(w).ok())
            .collect();
        info!(
            "Listed {} weight entries ({} valid).",
            entries.len(),
            metrics.len()
        );

        Ok(metrics)
    }

    async fn fetch_weight_range(
        client: &GarminClient,
        oauth2: &OAuth2Token,
        start_date: chrono::NaiveDate,
        end_date: chrono::NaiveDate,
    ) -> Result<Vec<WeightEntry>> {
        let path = format!(
            "/weight-service/weight/dateRange?startDate={}&endDate={}",
            start_date.format("%Y-%m-%d"),
            end_date.format("%Y-%m-%d")
        );
        let result: WeightRange = with_retry(|| client.get_json(oauth2, &path))
            .await
            .map_err(|e| log_error("Error fetching weight", ConnectError::FetchWeight(e)))?;

        Ok(result.date_weight_list)
    }

    async fn download_activity(
        client: &GarminClient,
        oauth2: &OAuth2Token,
        id: u64,
        dst_path: &Path,
    ) -> Result<()> {
        info!("Downloading activity {}...", id);
        let path = format!("/download-service/files/activity/{}", id);
        let zipped = with_retry(|| client.download(oauth2, &path))
            .await
            .map_err(|e| {
                log_error(
                    "Error downloading activity",
                    ConnectError::DownloadActivity(id, e),
                )
            })?;

        let dst_path = dst_path.to_path_buf();
        tokio::task::spawn_blocking(move || Self::extract_activity(id, zipped, &dst_path))
            .await
            .map_err(|e| {
                log_error(
                    "Error joining extraction task",
                    ConnectError::ExtractTask(id, e),
                )
            })?
    }

    async fn fetch_workouts(
        client: &GarminClient,
        oauth2: &OAuth2Token,
    ) -> Result<Vec<WorkoutSummary>> {
        let mut workouts = Vec::new();
        loop {
            let path = format!(
                "/workout-service/workouts?start={}&limit={}",
                workouts.len(),
                WORKOUT_LIST_STEP
            );
            let result: Vec<WorkoutSummary> = with_retry(|| client.get_json(oauth2, &path))
                .await
                .map_err(|e| {
                log_error(
                    "Error fetching workout page",
                    ConnectError::FetchWorkouts(e),
                )
            })?;
            let page_len = result.len();
            workouts.extend(result);

            if page_len < WORKOUT_LIST_STEP as usize {
                break;
            }
        }

        Ok(workouts)
    }

    fn extract_activity(id: u64, zipped: Bytes, dst_path: &Path) -> Result<()> {
        let mut archive = zip::ZipArchive::new(Cursor::new(zipped)).map_err(|e| {
            log_error(
                "Error opening activity archive",
                ConnectError::UnzipActivity(id, e),
            )
        })?;
        for i in 0..archive.len() {
            let mut entry = archive.by_index(i).map_err(|e| {
                log_error(
                    "Error reading activity archive entry",
                    ConnectError::UnzipActivity(id, e),
                )
            })?;
            let Some(file_name) = entry
                .enclosed_name()
                .and_then(|p| p.file_name().map(|n| n.to_owned()))
            else {
                continue;
            };

            if !file_name
                .to_string_lossy()
                .to_ascii_lowercase()
                .ends_with(".fit")
            {
                continue;
            }

            let mut fit = Vec::with_capacity(entry.size() as usize);
            entry.read_to_end(&mut fit).map_err(|e| {
                log_error(
                    "Error extracting activity",
                    ConnectError::ExtractActivity(id, e),
                )
            })?;
            std::fs::write(dst_path.join(file_name), fit).map_err(|e| {
                log_error("Error writing activity", ConnectError::WriteActivity(id, e))
            })?;
        }

        Ok(())
    }

    fn read_keyring<T: DeserializeOwned>(key: &str) -> Result<Option<T>> {
        let entry = Self::keyring_entry(key)?;
        match entry.get_password() {
            Ok(json) => Ok(Some(serde_json::from_str(&json).map_err(|e| {
                log_error(
                    &format!("Error deserializing token '{}'", key),
                    ConnectError::Serialization(e),
                )
            })?)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(log_error(
                &format!("Error reading token '{}' from keyring", key),
                ConnectError::Keyring(e),
            )),
        }
    }

    fn write_keyring<T: Serialize>(key: &str, value: &T) -> Result<()> {
        let json = serde_json::to_string(value).map_err(|e| {
            log_error(
                &format!("Error serializing key '{}'", key),
                ConnectError::Serialization(e),
            )
        })?;
        Self::keyring_entry(key)?.set_password(&json).map_err(|e| {
            log_error(
                &format!("Error writing key '{}' to keyring", key),
                ConnectError::Keyring(e),
            )
        })
    }

    fn delete_keyring(key: &str) -> Result<()> {
        match Self::keyring_entry(key)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(log_error(
                &format!("Error deleting key '{}' from keyring", key),
                ConnectError::Keyring(e),
            )),
        }
    }

    fn keyring_entry(key: &str) -> Result<keyring::Entry> {
        keyring::Entry::new(&constants::APP_NAME, key).map_err(|e| {
            log_error(
                &format!("Error opening keyring entry '{}'", key),
                ConnectError::Keyring(e),
            )
        })
    }
}

async fn with_retry<T, F, Fut>(mut op: F) -> std::result::Result<T, GarminError>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = std::result::Result<T, GarminError>>,
{
    let mut attempt = 1;
    let mut delay = RETRY_BASE_DELAY;
    loop {
        match op().await {
            Err(e) if attempt < RETRY_ATTEMPTS && is_retryable(&e) => {
                warn!(
                    "Transient Garmin error ({}), retrying in {:?} ({}/{})",
                    e, delay, attempt, RETRY_ATTEMPTS
                );
                tokio::time::sleep(delay).await;
                attempt += 1;
                delay *= 2;
            }
            result => return result,
        }
    }
}

fn is_retryable(err: &GarminError) -> bool {
    match err {
        GarminError::RateLimited => true,
        GarminError::Api { status, .. } => *status >= 500,
        GarminError::Http(curl_rest::Error::Client(e)) => {
            e.is_operation_timedout() || e.is_couldnt_connect() || e.is_couldnt_resolve_host()
        }
        _ => false,
    }
}

fn log_error(line: &str, err: ConnectError) -> ConnectError {
    error!("{}: {}", line, err);
    err
}

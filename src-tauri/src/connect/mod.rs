#[allow(dead_code, unused_imports)]
pub mod client;
pub mod errors;

use crate::{
    connect::{
        client::{
            CalendarItem, CalendarItemType, DeviceMessage, DeviceSummary, ExerciseSets,
            GarminClient, GarminError, OAuth1Token, OAuth2Token, SsoClient, WeightEntry,
            WeightRange, WorkoutSummary,
        },
        errors::{ConnectError, Result},
    },
    dao::body_metric::BodyMetric,
    fit::FitFile,
    utils::constants,
};
use bytes::Bytes;
use chrono::{Datelike, Local, NaiveDate, TimeZone};
use futures_util::{
    future::BoxFuture,
    stream::{self, StreamExt, TryStreamExt},
};
use serde::{Serialize, de::DeserializeOwned};
use std::{
    io::{Cursor, Read},
    sync::Arc,
    time::Duration,
};

use tauri_plugin_log::log::{error, info, warn};

const KEYRING_EMAIL: &str = "garmin-email";
const KEYRING_OAUTH1: &str = "garmin-oauth1";
const KEYRING_OAUTH2: &str = "garmin-oauth2";

const DOWNLOAD_CONCURRENCY: usize = 6;
const WORKOUT_LIST_STEP: u8 = 100;
const WEIGHT_CHUNK_MONTHS: u32 = 12;
const WEIGHT_EMPTY_CHUNKS_STOP: u32 = 2;
const RETRY_ATTEMPTS: u32 = 4;
const RETRY_BASE_DELAY: Duration = Duration::from_secs(2);
const TOKEN_EXPIRY_MARGIN_SECS: i64 = 300;
const CALENDAR_RECENT_DAYS: u64 = 15;

/// Year and month (1-12) of every calendar page covering the days from `from` to `to`.
pub fn calendar_months(from: NaiveDate, to: NaiveDate) -> Vec<(u16, u8)> {
    let mut months = Vec::new();
    let (mut year, mut month) = (from.year(), from.month());
    while (year, month) <= (to.year(), to.month()) {
        months.push((year as u16, month as u8));
        (year, month) = if month == 12 {
            (year + 1, 1)
        } else {
            (year, month + 1)
        };
    }
    months
}

/// Items of the page of `month` that fall between `from` and `to`. A page also holds days of the
/// adjacent months, which are left to their own pages so no item is listed twice.
pub fn calendar_page_items(
    page: Vec<CalendarItem>,
    year: u16,
    month: u8,
    from: NaiveDate,
    to: NaiveDate,
) -> Vec<CalendarItem> {
    page.into_iter()
        .filter(|item| {
            item.date.year() == i32::from(year)
                && item.date.month() == u32::from(month)
                && (from..=to).contains(&item.date)
        })
        .collect()
}

/// Id of the activity of `items` that started at `timestamp`, as sessions are keyed by their start.
pub fn session_activity_id(items: &[CalendarItem], timestamp: u32) -> Option<u64> {
    items
        .iter()
        .find(|item| {
            item.item_type == CalendarItemType::Activity
                && item.start_timestamp_local == Some(timestamp)
        })
        .and_then(|item| item.id)
}

/// Asks the user for the inputs a Garmin Connect login needs. Each future resolves to `None` if
/// the user cancels.
pub trait ConnectPrompter: Send + Sync {
    /// Email and password, with the email of the last attempt and the error it failed with.
    fn credentials(
        &self,
        email: Option<String>,
        error: Option<String>,
    ) -> BoxFuture<'static, Option<(String, String)>>;

    /// MFA verification code, sent through `method` (e.g. `email`).
    fn mfa_code(&self, method: String) -> BoxFuture<'static, Option<String>>;
}

pub struct ConnectClient {
    /// Called with the logged user every time it changes.
    on_user_change: Box<dyn Fn(Option<String>) + Send + Sync>,
    prompter: Arc<dyn ConnectPrompter>,
    email: Option<String>,
    oauth1: Option<OAuth1Token>,
    oauth2: Option<OAuth2Token>,
}

impl ConnectClient {
    pub fn new(
        on_user_change: impl Fn(Option<String>) + Send + Sync + 'static,
        prompter: impl ConnectPrompter + 'static,
    ) -> Self {
        Self {
            on_user_change: Box::new(on_user_change),
            prompter: Arc::new(prompter),
            email: Self::read_keyring(KEYRING_EMAIL).unwrap_or(None),
            oauth1: Self::read_keyring(KEYRING_OAUTH1).unwrap_or(None),
            oauth2: Self::read_keyring(KEYRING_OAUTH2).unwrap_or(None),
        }
    }

    pub fn logged_user(&self) -> Option<String> {
        self.email.clone()
    }

    pub async fn disconnect(&mut self) -> Result<()> {
        Self::delete_keyring(KEYRING_OAUTH2).await?;
        self.oauth2 = None;
        Self::delete_keyring(KEYRING_OAUTH1).await?;
        self.oauth1 = None;
        Self::delete_keyring(KEYRING_EMAIL).await?;
        self.email = None;

        (self.on_user_change)(None);

        Ok(())
    }

    pub async fn login(&mut self, email: &str, password: &str) -> Result<()> {
        info!("Logging in Garmin Connect for user {}...", email);
        let mut sso = SsoClient::new()
            .map_err(|e| log_error("Error creating SSO client", ConnectError::Login(e)))?;

        let prompter = self.prompter.clone();
        let (oauth1, oauth2) = sso
            .login(
                email,
                password,
                Some(move |method: String| {
                    info!("Garmin Connect requested MFA via {}", method);
                    prompter.mfa_code(method)
                }),
            )
            .await
            .map_err(|e| match e {
                GarminError::MfaCancelled => {
                    warn!("Garmin Connect MFA cancelled by the user");
                    ConnectError::LoginCancelledByUser()
                }
                e => log_error("Error logging in", ConnectError::Login(e)),
            })?;

        Self::write_keyring(KEYRING_EMAIL, &email.to_string()).await?;
        Self::write_keyring(KEYRING_OAUTH1, &oauth1).await?;
        Self::write_keyring(KEYRING_OAUTH2, &oauth2).await?;
        self.email = Some(email.to_string());
        self.oauth1 = Some(oauth1);
        self.oauth2 = Some(oauth2);

        info!("Logged in succesfully");
        (self.on_user_change)(Some(email.to_string()));

        Ok(())
    }

    /// Asks for the credentials until the login succeeds or the user cancels.
    pub async fn prompt_login(&mut self) -> Result<()> {
        let mut email = self.email.clone();
        let mut error = None;
        loop {
            let Some((entered_email, password)) =
                self.prompter.credentials(email.take(), error.take()).await
            else {
                warn!("Garmin Connect login cancelled by the user");
                return Err(ConnectError::LoginCancelledByUser());
            };

            match self.login(&entered_email, &password).await {
                Ok(()) => return Ok(()),
                Err(e @ ConnectError::LoginCancelledByUser()) => return Err(e),
                // Shown on the next prompt, along with the email to retry with
                Err(e) => {
                    email = Some(entered_email);
                    error = Some(e.to_string());
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

        // Never logged in: the login is only started on demand from the settings
        let Some(oauth1) = &self.oauth1 else {
            return Err(ConnectError::LoginRequired());
        };

        info!("Refreshing OAuth2 token...");
        {
            let sso = SsoClient::with_domain(oauth1.domain.clone()).map_err(|e| {
                log_error("Error creating SSO client", ConnectError::OAuthRefresh(e))
            })?;
            match with_retry(|| sso.refresh_oauth2(oauth1)).await {
                Ok(oauth2) => {
                    Self::write_keyring(KEYRING_OAUTH2, &oauth2).await?;
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

    /// Downloads into memory the `.fit` files of the activities with the given ids.
    pub async fn download_activities(&mut self, ids: &[u64]) -> Result<Vec<FitFile>> {
        if ids.is_empty() {
            info!("Activities up to date");
            return Ok(Vec::new());
        }

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

        info!("Downloading {} activities...", ids.len());
        let files: Vec<Vec<FitFile>> = stream::iter(ids.iter().copied())
            .map(|id| Self::download_activity(&client, oauth2, id))
            .buffer_unordered(DOWNLOAD_CONCURRENCY)
            .try_collect()
            .await?;
        info!("Downloaded {} activities.", ids.len());

        Ok(files.into_iter().flatten().collect())
    }

    /// Lists every device registered to the Connect account.
    pub async fn list_devices(&mut self) -> Result<Vec<DeviceSummary>> {
        info!("Listing devices...");

        self.refresh_token().await?;

        let (Some(oauth1), Some(oauth2)) = (&self.oauth1, &self.oauth2) else {
            return Err(ConnectError::LoginRequired());
        };

        let client = GarminClient::new(&oauth1.domain).map_err(|e| {
            log_error(
                "Error creating Garmin client",
                ConnectError::FetchDevices(e),
            )
        })?;

        let devices = with_retry(|| client.list_devices(oauth2))
            .await
            .map_err(|e| log_error("Error fetching devices", ConnectError::FetchDevices(e)))?;
        info!("Listed {} devices.", devices.len());

        Ok(devices)
    }

    /// Lists the calendar items of the last `CALENDAR_RECENT_DAYS` days.
    pub async fn list_recent_calendar_items(&mut self) -> Result<Vec<CalendarItem>> {
        info!("Listing recent calendar items...");

        self.refresh_token().await?;

        let (Some(oauth1), Some(oauth2)) = (&self.oauth1, &self.oauth2) else {
            return Err(ConnectError::LoginRequired());
        };

        let client = GarminClient::new(&oauth1.domain).map_err(|e| {
            log_error(
                "Error creating Garmin client",
                ConnectError::FetchCalendar(e),
            )
        })?;

        let to = Local::now().date_naive();
        let from = to - chrono::Days::new(CALENDAR_RECENT_DAYS);

        let mut items = Vec::new();
        for (year, month) in calendar_months(from, to) {
            let page = with_retry(|| client.list_calendar_month(oauth2, year, month))
                .await
                .map_err(|e| {
                    log_error("Error fetching calendar", ConnectError::FetchCalendar(e))
                })?;
            items.extend(calendar_page_items(page, year, month, from, to));
        }
        info!("Listed {} calendar items.", items.len());

        Ok(items)
    }

    /// Id of the Connect activity of the session that started at `timestamp`, or `None` when
    /// Connect does not have it (yet, if the device has not synced it).
    pub async fn find_session_activity(&mut self, timestamp: u32) -> Result<Option<u64>> {
        info!("Looking for the activity of session {}...", timestamp);

        let date = Local
            .timestamp_opt(i64::from(timestamp), 0)
            .single()
            .ok_or_else(|| {
                log_error(
                    "Error parsing session timestamp",
                    ConnectError::InvalidTimestamp(timestamp),
                )
            })?
            .date_naive();
        let (year, month) = (date.year() as u16, date.month() as u8);

        let (client, oauth2) = self.api_client(ConnectError::FetchCalendar).await?;
        let page = with_retry(|| client.list_calendar_month(oauth2, year, month))
            .await
            .map_err(|e| log_error("Error fetching calendar", ConnectError::FetchCalendar(e)))?;

        let id = session_activity_id(
            &calendar_page_items(page, year, month, date, date),
            timestamp,
        );
        match id {
            Some(id) => info!("Session {} is activity {}.", timestamp, id),
            None => info!("Session {} not found in Connect.", timestamp),
        }

        Ok(id)
    }

    /// Notes (description) of the activity `id`.
    pub async fn get_activity_notes(&mut self, id: u64) -> Result<Option<String>> {
        info!("Fetching notes of activity {}...", id);

        let (client, oauth2) = self
            .api_client(|e| ConnectError::FetchActivity(id, e))
            .await?;
        let activity = with_retry(|| client.get_activity(oauth2, id))
            .await
            .map_err(|e| {
                log_error(
                    "Error fetching activity",
                    ConnectError::FetchActivity(id, e),
                )
            })?;
        info!("Fetched notes of activity {}.", id);

        Ok(activity.description.filter(|notes| !notes.is_empty()))
    }

    /// Replaces the notes (description) of the activity `id`; empty `notes` clear them.
    pub async fn update_activity_notes(&mut self, id: u64, notes: &str) -> Result<()> {
        info!("Updating notes of activity {}...", id);

        let (client, oauth2) = self
            .api_client(|e| ConnectError::UpdateActivity(id, e))
            .await?;
        // Retried: it replaces the description, so repeating it is harmless
        with_retry(|| client.update_activity_description(oauth2, id, notes))
            .await
            .map_err(|e| {
                log_error(
                    "Error updating activity",
                    ConnectError::UpdateActivity(id, e),
                )
            })?;
        info!("Updated notes of activity {}.", id);

        Ok(())
    }

    /// Sets of the strength activity `id`.
    pub async fn get_session_sets(&mut self, id: u64) -> Result<ExerciseSets> {
        info!("Fetching exercise sets of activity {}...", id);

        let (client, oauth2) = self
            .api_client(|e| ConnectError::FetchExerciseSets(id, e))
            .await?;
        let sets = with_retry(|| client.get_exercise_sets(oauth2, id))
            .await
            .map_err(|e| {
                log_error(
                    "Error fetching exercise sets",
                    ConnectError::FetchExerciseSets(id, e),
                )
            })?;
        info!(
            "Fetched {} exercise sets of activity {}.",
            sets.exercise_sets.len(),
            id
        );

        Ok(sets)
    }

    /// Replaces every set of the strength activity `sets.activity_id` with `sets`, which should
    /// come from `get_exercise_sets` so the fields not modelled are kept.
    pub async fn update_session_sets(&mut self, sets: &ExerciseSets) -> Result<()> {
        let id = sets.activity_id;
        info!("Updating exercise sets of activity {}...", id);

        let (client, oauth2) = self
            .api_client(|e| ConnectError::UpdateExerciseSets(id, e))
            .await?;
        // Retried: it replaces the whole list, so repeating it is harmless
        with_retry(|| client.update_exercise_sets(oauth2, sets))
            .await
            .map_err(|e| {
                log_error(
                    "Error updating exercise sets",
                    ConnectError::UpdateExerciseSets(id, e),
                )
            })?;
        info!("Updated exercise sets of activity {}.", id);

        Ok(())
    }

    /// Refreshes the token if needed and builds an API client for the domain of the account.
    /// `wrap` turns a client creation failure into the error of the calling operation.
    async fn api_client(
        &mut self,
        wrap: impl FnOnce(GarminError) -> ConnectError,
    ) -> Result<(GarminClient, &OAuth2Token)> {
        self.refresh_token().await?;

        let (Some(oauth1), Some(oauth2)) = (&self.oauth1, &self.oauth2) else {
            return Err(ConnectError::LoginRequired());
        };

        let client = GarminClient::new(&oauth1.domain)
            .map_err(|e| log_error("Error creating Garmin client", wrap(e)))?;

        Ok((client, oauth2))
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

    /// Queues the Connect workout `workout_id`, named `name`, for delivery to the device `unit_id`
    /// on its next sync.
    pub async fn send_workout_to_device(
        &mut self,
        workout_id: u64,
        name: &str,
        unit_id: u64,
    ) -> Result<()> {
        info!("Sending workout {} to device {}...", workout_id, unit_id);

        self.refresh_token().await?;

        let (Some(oauth1), Some(oauth2)) = (&self.oauth1, &self.oauth2) else {
            return Err(ConnectError::LoginRequired());
        };

        let client = GarminClient::new(&oauth1.domain).map_err(|e| {
            log_error(
                "Error creating Garmin client",
                ConnectError::SendWorkout(workout_id, unit_id, e),
            )
        })?;

        let message = [DeviceMessage {
            device_id: unit_id,
            message_url: format!("workout-service/workout/FIT/{}", workout_id),
            message_type: "workouts",
            message_name: name,
            group_name: None,
            priority: 1,
            file_type: "FIT",
            meta_data_id: workout_id,
        }];

        // Not retried: a request that timed out after reaching Connect would queue the workout twice
        client
            .post_json::<_, serde_json::Value>(
                oauth2,
                "/device-service/devicemessage/messages",
                &message,
            )
            .await
            .map_err(|e| {
                log_error(
                    "Error sending workout",
                    ConnectError::SendWorkout(workout_id, unit_id, e),
                )
            })?;
        info!("Sent workout {} to device {}.", workout_id, unit_id);

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
    ) -> Result<Vec<FitFile>> {
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

        tokio::task::spawn_blocking(move || Self::extract_activity(id, zipped))
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

    /// Extracts into memory the `.fit` files of the archive Connect serves an activity as.
    fn extract_activity(id: u64, zipped: Bytes) -> Result<Vec<FitFile>> {
        let mut archive = zip::ZipArchive::new(Cursor::new(zipped)).map_err(|e| {
            log_error(
                "Error opening activity archive",
                ConnectError::UnzipActivity(id, e),
            )
        })?;
        let mut files = Vec::new();
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
            files.push(FitFile::new(file_name.to_string_lossy(), fit).with_activity_id(id));
        }

        Ok(files)
    }

    fn read_keyring<T: DeserializeOwned>(key: &str) -> Result<Option<T>> {
        // Tests must never see nor touch the credentials of the real user
        if cfg!(test) {
            return Ok(None);
        }

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

    /// Keyring calls go through D-Bus and can wait on the user unlocking it, so they run on the
    /// blocking thread pool.
    async fn write_keyring<T: Serialize>(key: &'static str, value: &T) -> Result<()> {
        if cfg!(test) {
            return Ok(());
        }

        let json = serde_json::to_string(value).map_err(|e| {
            log_error(
                &format!("Error serializing key '{}'", key),
                ConnectError::Serialization(e),
            )
        })?;
        tokio::task::spawn_blocking(move || {
            Self::keyring_entry(key)?.set_password(&json).map_err(|e| {
                log_error(
                    &format!("Error writing key '{}' to keyring", key),
                    ConnectError::Keyring(e),
                )
            })
        })
        .await
        .expect("Keyring task panicked")
    }

    async fn delete_keyring(key: &'static str) -> Result<()> {
        if cfg!(test) {
            return Ok(());
        }

        tokio::task::spawn_blocking(
            move || match Self::keyring_entry(key)?.delete_credential() {
                Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
                Err(e) => Err(log_error(
                    &format!("Error deleting key '{}' from keyring", key),
                    ConnectError::Keyring(e),
                )),
            },
        )
        .await
        .expect("Keyring task panicked")
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

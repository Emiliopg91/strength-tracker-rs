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
};
use bytes::Bytes;
use futures_util::stream::{self, TryStreamExt};
use serde::{Serialize, de::DeserializeOwned};
use std::{
    io::{Cursor, Read},
    path::Path,
    sync::LazyLock,
    time::Duration,
};
use tokio::sync::Mutex;

use tauri_plugin_log::log::{error, info, warn};

const KEYRING_SERVICE: &str = "com.emiliopg91.strength-tracker-rs";
const KEYRING_OAUTH1: &str = "garmin-oauth1";
const KEYRING_OAUTH2: &str = "garmin-oauth2";
const DOWNLOAD_STEP: u8 = 100;
const DOWNLOAD_CONCURRENCY: usize = 6;
const WEIGHT_CHUNK_MONTHS: u32 = 12;
const WEIGHT_EMPTY_CHUNKS_STOP: u32 = 2;
const RETRY_ATTEMPTS: u32 = 4;
const RETRY_BASE_DELAY: Duration = Duration::from_secs(2);
const TOKEN_EXPIRY_MARGIN_SECS: i64 = 300;

pub static CONNECT_CLIENT_INST: LazyLock<Mutex<ConnectClient>> =
    LazyLock::new(|| Mutex::new(ConnectClient::new().unwrap()));

pub struct ConnectClient {
    oauth1: Option<OAuth1Token>,
    oauth2: Option<OAuth2Token>,
}

impl ConnectClient {
    pub fn new() -> Result<Self> {
        Ok(Self {
            oauth1: Self::load_token(KEYRING_OAUTH1)?,
            oauth2: Self::load_token(KEYRING_OAUTH2)?,
        })
    }

    pub async fn login(&mut self, email: &str, password: &str) -> Result<()> {
        info!("Logging in Garmin Connect for user {}...", email);
        let mut sso = SsoClient::new()
            .map_err(|e| log_error("Error creating SSO client", ConnectError::Login(e)))?;

        let (oauth1, oauth2) = sso
            .login(email, password, None::<fn() -> String>)
            .await
            .map_err(|e| log_error("Error logging in", ConnectError::Login(e)))?;

        Self::store_token(KEYRING_OAUTH1, &oauth1)?;
        Self::store_token(KEYRING_OAUTH2, &oauth2)?;
        self.oauth1 = Some(oauth1);
        self.oauth2 = Some(oauth2);

        info!("Logged in succesfully");

        Ok(())
    }

    pub async fn prompt_login(&mut self) -> Result<bool> {
        let output = tokio::process::Command::new("zenity")
            .args([
                "--forms",
                "--title=Garmin Connect",
                "--text=Sign in to Garmin Connect",
                "--add-entry=Email",
                "--add-password=Password",
                "--separator=\n",
            ])
            .output()
            .await
            .map_err(|e| log_error("Error launching zenity", ConnectError::CredentialsPrompt(e)))?;

        // Cancelled by the user
        if !output.status.success() {
            info!("Garmin Connect login cancelled by the user");
            return Ok(false);
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stdout = stdout.strip_suffix('\n').unwrap_or(&stdout);
        let (email, password) = stdout.split_once('\n').unwrap_or((stdout, ""));

        self.login(email, password).await?;
        Ok(true)
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
                    Self::store_token(KEYRING_OAUTH2, &oauth2)?;
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
            Ok(true) => Ok(()),
            _ => Err(ConnectError::LoginRequired()),
        }
    }

    pub async fn download_activities<P>(&mut self, from: u32, dst_path: P) -> Result<()>
    where
        P: AsRef<Path>,
    {
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

        let mut activities = Vec::new();
        loop {
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
            activities.extend(result);

            if page_len < DOWNLOAD_STEP as usize {
                break;
            }
        }

        let dst_path = dst_path.as_ref();
        stream::iter(activities.iter().map(Ok))
            .try_for_each_concurrent(DOWNLOAD_CONCURRENCY, |activity| {
                Self::download_activity(&client, oauth2, activity.activity_id, dst_path)
            })
            .await
    }

    pub async fn list_weight(&mut self, from: Option<u32>) -> Result<Vec<BodyMetric>> {
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

        Ok(entries
            .iter()
            .filter_map(|w| BodyMetric::try_from(w).ok())
            .collect())
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

    fn load_token<T: DeserializeOwned>(key: &str) -> Result<Option<T>> {
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

    fn store_token<T: Serialize>(key: &str, token: &T) -> Result<()> {
        let json = serde_json::to_string(token).map_err(|e| {
            log_error(
                &format!("Error serializing token '{}'", key),
                ConnectError::Serialization(e),
            )
        })?;
        Self::keyring_entry(key)?.set_password(&json).map_err(|e| {
            log_error(
                &format!("Error writing token '{}' to keyring", key),
                ConnectError::Keyring(e),
            )
        })
    }

    fn keyring_entry(key: &str) -> Result<keyring::Entry> {
        keyring::Entry::new(KEYRING_SERVICE, key).map_err(|e| {
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
        GarminError::Http(e) => e.is_timeout() || e.is_connect(),
        _ => false,
    }
}

fn log_error(line: &str, err: ConnectError) -> ConnectError {
    error!("{}: {}", line, err);
    err
}

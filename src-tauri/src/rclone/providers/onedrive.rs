use std::ffi::OsStr;

use curl_rest::StatusCode;
use serde_json::Value;
use tauri_plugin_log::log::info;

use crate::rclone::{
    RCloneClient, RCloneTrait,
    errors::{self, RCloneError},
    providers::OneDrive,
};

impl RCloneTrait for RCloneClient<OneDrive> {
    fn get_provider_suffix() -> String {
        "onedrive".to_string()
    }

    async fn configure(&self) -> errors::Result<()> {
        info!("Getting authorization for OneDrive...");
        let output = Self::run_rclone(
            &[OsStr::new("authorize"), OsStr::new("onedrive")],
            RCloneError::Authorization,
        )
        .await?;

        let stdout = String::from_utf8_lossy(&output.stdout);

        let token_line = stdout
            .lines()
            .find(|l| l.starts_with("{"))
            .ok_or_else(RCloneError::BadToken)?;
        let json_obj =
            serde_json::from_str::<Value>(token_line).map_err(RCloneError::BadJsonFormat)?;
        let bearer_token = json_obj
            .get("access_token")
            .and_then(|v| v.as_str())
            .ok_or(RCloneError::MissingJsonField("access_token"))?
            .to_string();

        info!("Getting available drives...");
        let resp = tokio::task::spawn_blocking(move || {
            curl_rest::Client::default()
                .get()
                .header(curl_rest::Header::Authorization(
                    format!("Bearer {bearer_token}").into(),
                ))
                .header(curl_rest::Header::Accept("application/json".into()))
                .send("https://graph.microsoft.com/v1.0/me/drives")
        })
        .await
        .map_err(RCloneError::TaskError)?
        .map_err(RCloneError::ErrorGettingDrives)?;

        if resp.status != StatusCode::Ok {
            return Err(RCloneError::BadResponseStatus(resp.status));
        }

        let json_obj = serde_json::from_slice::<Value>(resp.body.as_slice())
            .map_err(RCloneError::BadJsonFormat)?;
        let drive = json_obj
            .get("value")
            .and_then(|v| v.as_array())
            .into_iter()
            .flatten()
            .find_map(|v| {
                if v.get("driveType").unwrap_or_default() == "personal"
                    && v.get("name").unwrap_or_default() == "OneDrive"
                {
                    v.get("id").and_then(|id| id.as_str())
                } else {
                    None
                }
            })
            .ok_or_else(RCloneError::NoDriveFound)?;

        info!("Selected drive with ID {drive}");
        info!("Configuring rclone...");
        Self::run_rclone(
            &[
                OsStr::new("config"),
                OsStr::new("create"),
                OsStr::new(&Self::get_provider_key()),
                OsStr::new("onedrive"),
                OsStr::new("token"),
                OsStr::new(token_line),
                OsStr::new("drive_id"),
                OsStr::new(drive),
                OsStr::new("drive_type"),
                OsStr::new("personal"),
                OsStr::new("--non-interactive"),
            ],
            RCloneError::CommandError,
        )
        .await?;

        info!("Remote 'onedrive' configured");
        Ok(())
    }
}

use strength_tracker_rs_macros::traced_command;
use tauri::{AppHandle, State};

use crate::{SettingsLock, rclone::providers::CloudProvider};

/// Uploads the database file to `provider` through rclone, configuring the remote first if needed
/// (which opens the provider's login in the browser). Notifies on success. On failure, shows a
/// persistent error notification and rejects with the message.
#[traced_command]
#[tauri::command]
pub async fn upload_to_cloud(
    settings: State<'_, SettingsLock>,
    provider: CloudProvider,
) -> Result<(), String> {
    super::upload_to_cloud(&settings, provider).await
}

/// Asks where to save the whole database as JSON (defaulting to a timestamped file in the home dir)
/// and writes it there, notifying the path. Resolves without doing anything if the dialog is
/// cancelled. On failure, shows a persistent error notification and rejects with the message.
#[traced_command]
#[tauri::command]
pub async fn export_database(
    app: AppHandle,
    settings: State<'_, SettingsLock>,
) -> Result<(), String> {
    super::export_database(&app, &settings).await
}

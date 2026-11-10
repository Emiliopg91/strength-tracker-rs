use strength_tracker_rs_macros::traced_command;
use tauri::{AppHandle, State};

use crate::{SettingsLock, rclone::providers::CloudProvider};

#[traced_command]
#[tauri::command]
pub async fn upload_to_cloud(
    settings: State<'_, SettingsLock>,
    provider: CloudProvider,
) -> Result<(), String> {
    super::upload_to_cloud(&settings, provider).await
}

#[traced_command]
#[tauri::command]
pub async fn export_database(
    app: AppHandle,
    settings: State<'_, SettingsLock>,
) -> Result<(), String> {
    super::export_database(&app, &settings).await
}

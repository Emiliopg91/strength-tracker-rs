use strength_tracker_rs_macros::traced_command;
use tauri::{AppHandle, State};

use crate::{
    SettingsLock,
    dto::sessions::{SessionDetails, SessionListItem, SessionStepsUpdate},
};

// Tauri commands: thin entry points over the logic of each module, which is what tests call
#[traced_command]
#[tauri::command]
pub async fn get_sessions(
    app: AppHandle,
    settings: State<'_, SettingsLock>,
    limit: Option<i32>,
) -> Result<Vec<SessionListItem>, String> {
    super::get_sessions(&app, &settings, limit).await
}

#[traced_command]
#[tauri::command]
pub async fn get_session_details(
    app: AppHandle,
    settings: State<'_, SettingsLock>,
    timestamp: u32,
) -> Result<SessionDetails, String> {
    super::get_session_details(&app, &settings, timestamp).await
}

#[traced_command]
#[tauri::command]
pub async fn download_session(
    app: AppHandle,
    settings: State<'_, SettingsLock>,
    timestamp: u32,
) -> Result<(), String> {
    super::download_session(&app, &settings, timestamp).await
}

#[traced_command]
#[tauri::command]
pub async fn save_session_changes(
    app: AppHandle,
    settings: State<'_, SettingsLock>,
    details: SessionStepsUpdate,
) -> Result<(), String> {
    super::save_session_changes(&app, &settings, details).await
}

#[traced_command]
#[tauri::command]
pub async fn import_from_files(
    app: AppHandle,
    settings: State<'_, SettingsLock>,
) -> Result<usize, String> {
    super::import_from_files(&app, &settings).await
}

#[traced_command]
#[tauri::command]
pub async fn get_heatmap_data(app: AppHandle) -> Result<Vec<Vec<(u32, u16)>>, String> {
    super::get_heatmap_data(&app).await
}

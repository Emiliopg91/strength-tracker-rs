use std::collections::HashMap;

use strength_tracker_rs_macros::traced_command;
use tauri::{AppHandle, State};

use crate::{
    SettingsLock,
    dto::app::{AppEnvironment, Settings},
};

#[traced_command]
#[tauri::command]
pub async fn get_settings(settings: State<'_, SettingsLock>) -> Result<Settings, String> {
    super::get_settings(&settings).await
}

#[traced_command]
#[tauri::command]
pub async fn notify_frontend_ready(app: AppHandle) {
    super::notify_frontend_ready(&app).await
}

#[traced_command]
#[tauri::command]
pub async fn get_environment() -> AppEnvironment {
    super::get_environment().await
}

#[traced_command]
#[tauri::command]
pub async fn update_settings_value(app: AppHandle, name: &str, value: &str) -> Result<(), String> {
    super::update_settings_value(&app, name, value).await
}

#[traced_command(log_payload = false)]
#[tauri::command]
pub async fn get_translations(
    settings: State<'_, SettingsLock>,
) -> Result<HashMap<String, String>, String> {
    super::get_translations(&settings).await
}

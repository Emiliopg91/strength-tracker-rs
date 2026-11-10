use std::collections::HashMap;

use strength_tracker_rs_macros::traced_command;
use tauri::{AppHandle, State};

use crate::{
    SettingsLock,
    dto::app::{AppEnvironment, Settings},
};

/// Current in-memory settings (units, language, tray and startup behaviour).
#[traced_command]
#[tauri::command]
pub async fn get_settings(settings: State<'_, SettingsLock>) -> Result<Settings, String> {
    super::get_settings(&settings).await
}

/// Signals that the main window has mounted. Shows it (unless set to start into the tray) and, in
/// the background: installs the udev rules if missing (exits the app if that fails), syncs the
/// Connect devices, starts the USB device watcher, sends pending workouts to Connect, geocodes
/// pending sessions and checks for updates.
#[traced_command]
#[tauri::command]
pub async fn notify_frontend_ready(app: AppHandle) {
    super::notify_frontend_ready(&app).await
}

/// Whether this is a debug or a release build.
#[traced_command]
#[tauri::command]
pub async fn get_environment() -> AppEnvironment {
    super::get_environment().await
}

/// Stores the setting `name` (a `settings_keys` constant) as `value`, given as text (`"true"` /
/// `"false"` for flags, the enum name for units and language), and applies its side effect:
/// autostart for `START_ON_BOOT`, udev rules for `ON_DEVICE_CONNECT` (asked for before storing, so
/// a cancelled password prompt leaves the setting untouched). Notifies on success. On failure,
/// shows a persistent error notification and rejects with the message.
#[traced_command]
#[tauri::command]
pub async fn update_settings_value(app: AppHandle, name: &str, value: &str) -> Result<(), String> {
    super::update_settings_value(&app, name, value).await
}

/// Every translation key resolved to the current language, to bootstrap the frontend i18n.
#[traced_command(log_payload = false)]
#[tauri::command]
pub async fn get_translations(
    settings: State<'_, SettingsLock>,
) -> Result<HashMap<String, String>, String> {
    super::get_translations(&settings).await
}

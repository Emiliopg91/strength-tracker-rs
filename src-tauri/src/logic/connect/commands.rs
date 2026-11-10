use strength_tracker_rs_macros::traced_command;
use tauri::{AppHandle, State};

use crate::{ConnectLock, SettingsLock};

/// Downloads and imports the Connect activities of the last 15 days not stored yet. Files that fail
/// to parse are kept as pending; new sessions are geocoded in the background. Resolves to the
/// number of sessions imported. On failure, shows a persistent error notification and rejects with
/// the message.
#[traced_command]
#[tauri::command]
pub async fn import_from_connect(
    app: AppHandle,
    settings: State<'_, SettingsLock>,
    connect: State<'_, ConnectLock>,
) -> Result<usize, String> {
    super::import_from_connect(&app, &settings, &connect).await
}

/// Imports the Connect weight entries logged since the newest stored measurement, skipping dates
/// already stored. Resolves to the number of entries added. On failure, shows a persistent error
/// notification and rejects with the message.
#[traced_command]
#[tauri::command]
pub async fn import_weight_from_connect(
    app: AppHandle,
    settings: State<'_, SettingsLock>,
    connect: State<'_, ConnectLock>,
) -> Result<usize, String> {
    super::import_weight_from_connect(&app, &settings, &connect).await
}

/// Name of the user logged into Connect, `null` if none.
#[traced_command]
#[tauri::command]
pub async fn connect_logged_user(
    connect: State<'_, ConnectLock>,
) -> Result<Option<String>, String> {
    super::connect_logged_user(&connect).await
}

/// Asks for the Connect credentials (and MFA code, if required) through the frontend prompts until
/// the login succeeds or the user cancels. On success, syncs the Connect devices and sends them the
/// pending workouts. Resolves to the logged user, `null` if cancelled.
#[traced_command]
#[tauri::command]
pub async fn connect_login(
    app: AppHandle,
    connect: State<'_, ConnectLock>,
) -> Result<Option<String>, String> {
    super::connect_login(&app, &connect).await
}

/// Logs out of Connect, discarding the stored session.
#[traced_command]
#[tauri::command]
pub async fn connect_logout(connect: State<'_, ConnectLock>) -> Result<(), String> {
    super::connect_logout(&connect).await
}

/// Queues `workout` in Connect for delivery to the device `serial`, uploading it first if Connect
/// doesn't have it yet, and registers the pair so later changes are re-sent. Notifies on success.
/// On failure, shows a persistent error notification and rejects with the message.
#[traced_command]
#[tauri::command]
pub async fn send_to_connect(
    app: AppHandle,
    settings: State<'_, SettingsLock>,
    workout: &str,
    serial: &str,
) -> Result<(), String> {
    super::send_to_connect(&app, &settings, workout, serial).await
}

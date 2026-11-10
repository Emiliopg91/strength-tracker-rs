use strength_tracker_rs_macros::traced_command;
use tauri::{AppHandle, State};

use crate::{ConnectLock, SettingsLock};

#[traced_command]
#[tauri::command]
pub async fn import_from_connect(
    app: AppHandle,
    settings: State<'_, SettingsLock>,
    connect: State<'_, ConnectLock>,
) -> Result<usize, String> {
    super::import_from_connect(&app, &settings, &connect).await
}

#[traced_command]
#[tauri::command]
pub async fn import_weight_from_connect(
    app: AppHandle,
    settings: State<'_, SettingsLock>,
    connect: State<'_, ConnectLock>,
) -> Result<usize, String> {
    super::import_weight_from_connect(&app, &settings, &connect).await
}

#[traced_command]
#[tauri::command]
pub async fn connect_logged_user(
    connect: State<'_, ConnectLock>,
) -> Result<Option<String>, String> {
    super::connect_logged_user(&connect).await
}

#[traced_command]
#[tauri::command]
pub async fn connect_login(
    app: AppHandle,
    connect: State<'_, ConnectLock>,
) -> Result<Option<String>, String> {
    super::connect_login(&app, &connect).await
}

#[traced_command]
#[tauri::command]
pub async fn connect_logout(connect: State<'_, ConnectLock>) -> Result<(), String> {
    super::connect_logout(&connect).await
}

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

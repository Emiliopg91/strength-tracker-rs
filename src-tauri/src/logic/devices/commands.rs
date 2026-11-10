use strength_tracker_rs_macros::traced_command;
use tauri::{AppHandle, State};

use crate::{SettingsLock, dto::devices::DeviceListItem};

#[traced_command]
#[tauri::command]
pub async fn send_to_device(
    app: AppHandle,
    settings: State<'_, SettingsLock>,
    workout: &str,
    serial: &str,
) -> Result<(), String> {
    super::send_to_device(&app, &settings, workout, serial).await
}

#[traced_command]
#[tauri::command]
pub async fn import_from_device(app: AppHandle, serial: &str) -> Result<usize, String> {
    super::_import_from_device(&app, serial).await
}

#[traced_command]
#[tauri::command]
pub async fn get_registered_devices(app: AppHandle) -> Result<Vec<DeviceListItem>, String> {
    super::get_registered_devices(&app).await
}

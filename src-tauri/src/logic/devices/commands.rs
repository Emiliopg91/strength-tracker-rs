use strength_tracker_rs_macros::traced_command;
use tauri::{AppHandle, State};

use crate::{SettingsLock, dto::devices::DeviceListItem};

/// Sends `workout` over USB to the device `serial` if it's connected; otherwise queues it to be
/// sent the next time it's plugged in. Either way the pair is registered so later changes are
/// re-sent, and the outcome is notified. On failure, shows a persistent error notification and
/// rejects with the message.
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

/// Downloads over USB the activities recorded by the device `serial` since its last sync and
/// imports them, moving the sync mark. Files that fail to parse are kept as pending; new sessions
/// are geocoded in the background. Resolves to the number of sessions imported. On failure, shows a
/// persistent error notification and rejects with the message.
#[traced_command]
#[tauri::command]
pub async fn import_from_device(app: AppHandle, serial: &str) -> Result<usize, String> {
    super::import_from_device(&app, serial).await
}

/// Every registered device, leaving out the Connect pseudo-device.
#[traced_command]
#[tauri::command]
pub async fn get_registered_devices(app: AppHandle) -> Result<Vec<DeviceListItem>, String> {
    super::get_registered_devices(&app).await
}

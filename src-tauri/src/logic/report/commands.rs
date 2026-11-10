use strength_tracker_rs_macros::traced_command;
use tauri::{AppHandle, State, WebviewWindow};

use crate::{SettingsLock, dto::report::ReportData};

#[traced_command]
#[tauri::command]
pub async fn load_report(app: AppHandle, from: u32, to: u32) -> Result<(), String> {
    super::load_report(&app, from, to).await
}

#[traced_command]
#[tauri::command]
pub async fn report_ready(window: WebviewWindow) -> Result<(), String> {
    super::report_ready(window).await
}

#[traced_command(log_payload = false)]
#[tauri::command]
pub async fn get_report_data(
    app: AppHandle,
    settings: State<'_, SettingsLock>,
    from: u32,
    to: u32,
) -> Result<ReportData, String> {
    super::get_report_data(&app, &settings, from, to).await
}

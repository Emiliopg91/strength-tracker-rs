use strength_tracker_rs_macros::traced_command;
use tauri::{AppHandle, State, WebviewWindow};

use crate::{SettingsLock, dto::report::ReportData};

/// Opens the hidden report window for the range `from`..`to` (Unix seconds). It prints itself to
/// PDF once it calls `report_ready`.
#[traced_command]
#[tauri::command]
pub async fn load_report(app: AppHandle, from: u32, to: u32) -> Result<(), String> {
    super::load_report(&app, from, to).await
}

/// Called by the report window once rendered: asks where to save the PDF (defaulting to the
/// downloads dir), prints it there and opens it. The window is closed when done, on cancel or on
/// error.
#[traced_command]
#[tauri::command]
pub async fn report_ready(window: WebviewWindow) -> Result<(), String> {
    super::report_ready(window).await
}

/// Strength sessions in `from`..`to` (Unix seconds), the history in that range of every exercise
/// trained in them (sorted by translated name) and the body measurements of the period. On failure,
/// shows a persistent error notification and rejects with the message.
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

use strength_tracker_rs_macros::traced_command;
use tauri::{AppHandle, State};

use crate::{SettingsLock, dto::body_metrics::BodyMetricListItem};

/// Every logged body measurement, newest first. On failure, shows a persistent error notification
/// and rejects with the message.
#[traced_command]
#[tauri::command]
pub async fn get_body_measures(
    app: AppHandle,
    settings: State<'_, SettingsLock>,
) -> Result<Vec<BodyMetricListItem>, String> {
    super::get_body_measures(&app, &settings).await
}

/// Stores a new body measurement entry. On failure, shows a persistent error notification and
/// rejects with the message.
#[traced_command]
#[tauri::command]
pub async fn add_body_measures(
    app: AppHandle,
    settings: State<'_, SettingsLock>,
    measures: BodyMetricListItem,
) -> Result<(), String> {
    super::add_body_measures(&app, &settings, measures).await
}

/// Deletes the body measurement logged at `date` (Unix seconds), notifying on success. On failure,
/// shows a persistent error notification and rejects with the message.
#[traced_command]
#[tauri::command]
pub async fn delete_body_metric(
    app: AppHandle,
    settings: State<'_, SettingsLock>,
    date: u32,
) -> Result<(), String> {
    super::delete_body_metric(&app, &settings, date).await
}

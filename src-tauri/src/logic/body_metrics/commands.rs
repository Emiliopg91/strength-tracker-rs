use strength_tracker_rs_macros::traced_command;
use tauri::{AppHandle, State};

use crate::{SettingsLock, dto::body_metrics::BodyMetricListItem};

#[traced_command]
#[tauri::command]
pub async fn get_body_measures(
    app: AppHandle,
    settings: State<'_, SettingsLock>,
) -> Result<Vec<BodyMetricListItem>, String> {
    super::get_body_measures(&app, &settings).await
}

#[traced_command]
#[tauri::command]
pub async fn add_body_measures(
    app: AppHandle,
    settings: State<'_, SettingsLock>,
    measures: BodyMetricListItem,
) -> Result<(), String> {
    super::add_body_measures(&app, &settings, measures).await
}

#[traced_command]
#[tauri::command]
pub async fn delete_body_metric(
    app: AppHandle,
    settings: State<'_, SettingsLock>,
    date: u32,
) -> Result<(), String> {
    super::delete_body_metric(&app, &settings, date).await
}

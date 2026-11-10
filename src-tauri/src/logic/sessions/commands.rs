use strength_tracker_rs_macros::traced_command;
use tauri::{AppHandle, State};

use crate::{
    SettingsLock,
    dto::sessions::{SessionDetails, SessionListItem, SessionStepsUpdate},
};

// Tauri commands: thin entry points over the logic of each module, which is what tests call
/// Every session, newest first, flagged with whether it has sets and records. Despite its name,
/// `limit` is the earliest session date (Unix seconds) to include; `null` lists them all. On
/// failure, shows a persistent error notification and rejects with the message.
#[traced_command]
#[tauri::command]
pub async fn get_sessions(
    app: AppHandle,
    settings: State<'_, SettingsLock>,
    limit: Option<i32>,
) -> Result<Vec<SessionListItem>, String> {
    super::get_sessions(&app, &settings, limit).await
}

/// Full details of the session at `timestamp`: sets, laps, heart rate, GPS, speeds and device. If
/// it has no Connect id, looks it up in Connect in the background. On failure, shows a persistent
/// error notification and rejects with the message.
#[traced_command]
#[tauri::command]
pub async fn get_session_details(
    app: AppHandle,
    settings: State<'_, SettingsLock>,
    timestamp: u32,
) -> Result<SessionDetails, String> {
    super::get_session_details(&app, &settings, timestamp).await
}

/// Asks where to save the session at `timestamp` as a FIT file (defaulting to `<date> <name>.fit`
/// in the home dir) and writes it there, notifying the path. Resolves without doing anything if the
/// dialog is cancelled. On failure, shows a persistent error notification and rejects with the
/// message.
#[traced_command]
#[tauri::command]
pub async fn download_session(
    app: AppHandle,
    settings: State<'_, SettingsLock>,
    timestamp: u32,
) -> Result<(), String> {
    super::download_session(&app, &settings, timestamp).await
}

/// Stores the edited notes and sets (reps, weight, exercise) of a session and recomputes the
/// records of the exercises involved. Notifies the update, new records and new progression
/// suggestions, emits `progressions_available`, and syncs the sets to Connect in the background. On
/// failure, shows a persistent error notification and rejects with the message.
#[traced_command]
#[tauri::command]
pub async fn save_session_changes(
    app: AppHandle,
    settings: State<'_, SettingsLock>,
    details: SessionStepsUpdate,
) -> Result<(), String> {
    super::save_session_changes(&app, &settings, details).await
}

/// Asks for `.fit` files and imports them as sessions. Notifies the result (files that failed to
/// parse, sessions imported, new records and progressions) and emits `progressions_available`; new
/// sessions are geocoded in the background. Resolves to the number of sessions imported, `0` if the
/// dialog is cancelled. On failure, shows a persistent error notification and rejects with the
/// message.
#[traced_command]
#[tauri::command]
pub async fn import_from_files(
    app: AppHandle,
    settings: State<'_, SettingsLock>,
) -> Result<usize, String> {
    super::import_from_files(&app, &settings).await
}

/// Activity of the last 12 months as a 12x31 grid indexed by month and day (0-based): each cell
/// holds the summed training load and number of records of that day.
#[traced_command]
#[tauri::command]
pub async fn get_heatmap_data(app: AppHandle) -> Result<Vec<Vec<(u32, u16)>>, String> {
    super::get_heatmap_data(&app).await
}

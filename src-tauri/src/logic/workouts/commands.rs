use strength_tracker_rs_macros::traced_command;
use tauri::{AppHandle, State, WebviewWindow};

use crate::{
    ConnectLock, SettingsLock,
    dao::workout::Workout,
    dto::workouts::{WorkoutDetails, WorkoutListItem},
};

/// Every workout with its session stats (count, average time and load, latest date), by name. On
/// failure, shows a persistent error notification and rejects with the message.
#[traced_command]
#[tauri::command]
pub async fn get_workout_list(
    app: AppHandle,
    settings: State<'_, SettingsLock>,
) -> Result<Vec<WorkoutListItem>, String> {
    super::get_workout_list(&app, &settings).await
}

/// The workout `name` with its steps, worked muscles and every session with its volume. On failure,
/// shows a persistent error notification and rejects with the message.
#[traced_command]
#[tauri::command]
pub async fn get_workout_details(
    app: AppHandle,
    settings: State<'_, SettingsLock>,
    name: &str,
) -> Result<WorkoutDetails, String> {
    super::get_workout_details(&app, &settings, name).await
}

/// Enables or disables `workout`. Enabling pushes it to the connected USB devices and the Connect
/// devices that have it; disabling removes it from the connected USB devices.
#[traced_command]
#[tauri::command]
pub async fn set_workout_status(app: AppHandle, workout: &str, status: bool) -> Result<(), String> {
    super::set_workout_status(&app, workout, status).await
}

/// Creates `workout`, or replaces the notes and steps of the existing one when `edit`, and uploads
/// it to Connect. When editing, it is also pushed to the connected USB devices that have it.
/// Notifies on success. On failure, shows a persistent error notification and rejects with the
/// message.
#[traced_command]
#[tauri::command]
pub async fn save_workout(
    app: AppHandle,
    settings: State<'_, SettingsLock>,
    connect: State<'_, ConnectLock>,
    edit: bool,
    workout: Workout,
) -> Result<(), String> {
    super::save_workout(&app, &settings, &connect, edit, workout).await
}

/// Opens the hidden workout window for `workout`. It prints itself to PDF once it calls
/// `workout_print_ready`.
#[traced_command]
#[tauri::command]
pub async fn load_workout_page(app: AppHandle, workout: &str) -> Result<(), String> {
    super::load_workout_page(&app, workout).await
}

/// Called by the workout window once rendered: asks where to save the PDF (defaulting to
/// `<workout>.pdf` in the downloads dir), prints it there and opens it. The window is closed when
/// done, on cancel or on error.
#[traced_command]
#[tauri::command]
pub async fn workout_print_ready(window: WebviewWindow, workout: &str) -> Result<(), String> {
    super::workout_print_ready(window, workout).await
}

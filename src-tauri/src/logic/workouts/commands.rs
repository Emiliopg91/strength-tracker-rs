use strength_tracker_rs_macros::traced_command;
use tauri::{AppHandle, State, WebviewWindow};

use crate::{
    ConnectLock, SettingsLock,
    dao::workout::Workout,
    dto::workouts::{WorkoutDetails, WorkoutListItem},
};

#[traced_command]
#[tauri::command]
pub async fn get_workout_list(
    app: AppHandle,
    settings: State<'_, SettingsLock>,
) -> Result<Vec<WorkoutListItem>, String> {
    super::get_workout_list(&app, &settings).await
}

#[traced_command]
#[tauri::command]
pub async fn get_workout_details(
    app: AppHandle,
    settings: State<'_, SettingsLock>,
    name: &str,
) -> Result<WorkoutDetails, String> {
    super::get_workout_details(&app, &settings, name).await
}

#[traced_command]
#[tauri::command]
pub async fn set_workout_status(app: AppHandle, workout: &str, status: bool) -> Result<(), String> {
    super::set_workout_status(&app, workout, status).await
}

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

#[traced_command]
#[tauri::command]
pub async fn load_workout_page(app: AppHandle, workout: &str) -> Result<(), String> {
    super::load_workout_page(&app, workout).await
}

#[traced_command]
#[tauri::command]
pub async fn workout_print_ready(window: WebviewWindow, workout: &str) -> Result<(), String> {
    super::workout_print_ready(window, workout).await
}

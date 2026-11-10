use std::collections::HashMap;

use strength_tracker_rs_macros::traced_command;
use tauri::{AppHandle, State};

use crate::{
    ConnectLock, SettingsLock,
    dto::exercises::{ExerciseDetails, ExerciseListItem, ProgressionSuggestion},
};

#[traced_command]
#[tauri::command]
pub async fn get_exercises(
    app: AppHandle,
    settings: State<'_, SettingsLock>,
) -> Result<Vec<ExerciseListItem>, String> {
    super::get_exercises(&app, &settings).await
}

#[traced_command]
#[tauri::command]
pub async fn get_exercise_details(
    app: AppHandle,
    settings: State<'_, SettingsLock>,
    category: u16,
    id: u16,
) -> Result<ExerciseDetails, String> {
    super::get_exercise_details(&app, &settings, category, id).await
}

#[traced_command]
#[tauri::command]
pub async fn get_exercises_catalog(app: AppHandle) -> Result<HashMap<u16, Vec<u16>>, String> {
    super::get_exercises_catalog(&app).await
}

#[traced_command]
#[tauri::command]
pub async fn set_exercise_unilateral(
    app: AppHandle,
    settings: State<'_, SettingsLock>,
    category: u16,
    id: u16,
    value: bool,
) -> Result<(), String> {
    super::set_exercise_unilateral(&app, &settings, category, id, value).await
}

#[traced_command]
#[tauri::command]
pub async fn set_exercise_progression(
    app: AppHandle,
    settings: State<'_, SettingsLock>,
    category: u16,
    id: u16,
    value: bool,
) -> Result<(), String> {
    super::set_exercise_progression(&app, &settings, category, id, value).await
}

#[traced_command]
#[tauri::command]
pub async fn set_exercise_progression_parameters(
    app: AppHandle,
    settings: State<'_, SettingsLock>,
    category: u16,
    id: u16,
    reps: &str,
    weight: f32,
) -> Result<(), String> {
    super::set_exercise_progression_parameters(&app, &settings, category, id, reps, weight).await
}

#[traced_command]
#[tauri::command]
pub async fn get_progression_status(
    app: AppHandle,
    category: u16,
    id: u16,
) -> Result<Option<ProgressionSuggestion>, String> {
    super::get_progression_status(&app, category, id).await
}

#[traced_command]
#[tauri::command]
pub async fn get_progression_suggestion(
    app: AppHandle,
    category: u16,
    id: u16,
) -> Result<Option<ProgressionSuggestion>, String> {
    super::get_progression_suggestion(&app, category, id).await
}

#[traced_command]
#[tauri::command]
pub async fn available_any_progression_suggestions(app: AppHandle) -> Result<bool, String> {
    Ok(super::available_any_progression_suggestions(&app).await)
}

#[traced_command]
#[tauri::command]
pub async fn apply_progression_suggestion(
    app: AppHandle,
    settings: State<'_, SettingsLock>,
    connect: State<'_, ConnectLock>,
    category: u16,
    id: u16,
    suggestion: ProgressionSuggestion,
) -> Result<(), String> {
    super::apply_progression_suggestion(&app, &settings, &connect, category, id, suggestion).await
}

#[traced_command]
#[tauri::command]
pub async fn save_exercise_notes(
    app: AppHandle,
    settings: State<'_, SettingsLock>,
    connect: State<'_, ConnectLock>,
    category: u16,
    id: u16,
    notes: &str,
) -> Result<(), String> {
    super::save_exercise_notes(&app, &settings, &connect, category, id, notes).await
}

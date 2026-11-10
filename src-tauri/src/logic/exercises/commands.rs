use std::collections::HashMap;

use strength_tracker_rs_macros::traced_command;
use tauri::{AppHandle, State};

use crate::{
    ConnectLock, SettingsLock,
    dto::exercises::{ExerciseDetails, ExerciseListItem, ProgressionSuggestion},
};

/// Exercises with a personal record, annotated with it, plus those planned in some workout step
/// that have none yet. On failure, shows a persistent error notification and rejects with the
/// message.
#[traced_command]
#[tauri::command]
pub async fn get_exercises(
    app: AppHandle,
    settings: State<'_, SettingsLock>,
) -> Result<Vec<ExerciseListItem>, String> {
    super::get_exercises(&app, &settings).await
}

/// Personal record and full per-session set history of the exercise `category`/`id`. On failure,
/// shows a persistent error notification and rejects with the message.
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

/// Exercises that can be picked, as exercise ids by category. A category's generic exercise is left
/// out when the category has a real exercise with the same name.
#[traced_command]
#[tauri::command]
pub async fn get_exercises_catalog(app: AppHandle) -> Result<HashMap<u16, Vec<u16>>, String> {
    super::get_exercises_catalog(&app).await
}

/// Marks the exercise `category`/`id` as performed one side at a time, or not. On failure, shows a
/// persistent error notification and rejects with the message.
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

/// Enables or disables progression suggestions for the exercise `category`/`id`. On failure, shows
/// a persistent error notification and rejects with the message.
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

/// Sets the progression parameters of the exercise `category`/`id`: the rep range `reps` (e.g.
/// `"8-10-12"`) and the weight increment `weight`. On failure, shows a persistent error
/// notification and rejects with the message.
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

/// Highest load programmed for the exercise `category`/`id` across workouts, `null` if none.
#[traced_command]
#[tauri::command]
pub async fn get_progression_status(
    app: AppHandle,
    category: u16,
    id: u16,
) -> Result<Option<ProgressionSuggestion>, String> {
    super::get_progression_status(&app, category, id).await
}

/// Pending progression suggestion for the exercise `category`/`id`, `null` if none.
#[traced_command]
#[tauri::command]
pub async fn get_progression_suggestion(
    app: AppHandle,
    category: u16,
    id: u16,
) -> Result<Option<ProgressionSuggestion>, String> {
    super::get_progression_suggestion(&app, category, id).await
}

/// Whether any exercise has a progression suggestion pending. `false` if it can't be checked.
#[traced_command]
#[tauri::command]
pub async fn available_any_progression_suggestions(app: AppHandle) -> Result<bool, String> {
    Ok(super::available_any_progression_suggestions(&app).await)
}

/// Raises to `suggestion` the steps programming the exercise `category`/`id` below it, then
/// re-uploads the affected workouts to Connect and, if any step changed, pushes them to the
/// connected USB devices. On failure, shows a persistent error notification and rejects with the
/// message.
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

/// Stores the notes of the exercise `category`/`id` (blank clears them) and re-sends, to Connect
/// and the connected USB devices, the workouts whose steps inherit them.
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

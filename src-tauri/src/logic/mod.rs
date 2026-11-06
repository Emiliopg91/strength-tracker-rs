pub mod app;
pub mod body_metrics;
pub mod connect;
pub mod devices;
pub mod exercises;
pub mod export;
pub mod notifications;
pub mod report;
pub mod sessions;
pub mod workouts;

use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

use chrono::Local;

use rusqlite_orm::{
    database::DatabasePool,
    types::{row::Row, value::Value},
};
use strength_tracker_rs_macros::traced_command;
use tauri::{AppHandle, Manager, Runtime, State, WebviewWindow};
use tauri_plugin_log::log::{error, warn};

use crate::{
    ConnectLock, SettingsLock,
    dao::workout::Workout,
    dto::{
        app::{AppEnvironment, Settings},
        body_metrics::BodyMetricListItem,
        devices::DeviceListItem,
        exercises::{ExerciseDetails, ExerciseListItem, ProgressionSuggestion},
        notifications::{NotificationDefinition, NotificationKind},
        report::ReportData,
        sessions::{SessionDetails, SessionListItem, SessionStepsUpdate},
        workouts::{WorkoutDetails, WorkoutListItem},
    },
    fit::FitFile,
    logic::notifications::show_notification,
    rclone::providers::CloudProvider,
    utils::translations::{Languages, translate},
};

/// Logs `e` (prefixed by `log_msg`), fires a persistent desktop notification titled by the
/// translation key `error_key`, and returns the stringified error for the command's
/// `Result<T, String>`.
pub fn report_error<E: std::fmt::Display>(
    e: E,
    lang: Languages,
    error_key: &str,
    log_msg: &str,
) -> String {
    error!("{}: {}", log_msg, e);
    show_notification(NotificationDefinition {
        title: translate(error_key, lang),
        body: e.to_string(),
        kind: NotificationKind::Persistant,
    });
    e.to_string()
}

/// Runs `f` with the database pool on tokio's blocking thread pool, so synchronous DB and file
/// work never stalls an async worker thread.
pub async fn run_blocking<R, T, F>(app: &AppHandle<R>, f: F) -> T
where
    R: Runtime,
    F: FnOnce(&DatabasePool) -> T + Send + 'static,
    T: Send + 'static,
{
    let app = app.clone();
    tokio::task::spawn_blocking(move || f(&app.state::<DatabasePool>()))
        .await
        .expect("blocking DB task panicked")
}

/// Adds `extension` to `path` unless it already ends in it.
pub(crate) fn ensure_extension(mut path: PathBuf, extension: &str) -> PathBuf {
    if path
        .extension()
        .is_none_or(|ext| !ext.eq_ignore_ascii_case(extension))
    {
        path.as_mut_os_string().push(format!(".{extension}"));
    }
    path
}

/// Asks where to save a file, starting at `default_file`. Returns `None` if the user cancels.
pub(crate) async fn ask_save_path(
    default_file: &Path,
    filter: &str,
    extension: &str,
    title: &str,
) -> Result<Option<PathBuf>, String> {
    let output = tokio::process::Command::new("zenity")
        .args([
            "--file-selection",
            "--save",
            &format!("--filename={}", default_file.display()),
            &format!("--file-filter={filter}"),
            &format!("--title={title}"),
        ])
        .output()
        .await
        .map_err(|e| e.to_string())?;

    if !output.status.success() {
        return Ok(None);
    }

    let path = String::from_utf8(output.stdout).map_err(|e| e.to_string())?;
    Ok(Some(ensure_extension(
        PathBuf::from(path.trim_end_matches('\n')),
        extension,
    )))
}

/// Gives every transfer its own dir, so concurrent syncs of the same source don't share files.
static PENDING_DIR_SEQ: AtomicU64 = AtomicU64::new(0);

/// Returns a new dir under `root` for a single transfer of `serial`. The timestamp keeps it apart
/// from those left by previous runs, and the sequence from concurrent transfers.
pub(crate) fn new_pending_dir(root: &Path, serial: &str) -> PathBuf {
    root.join(serial).join(format!(
        "{}-{}",
        Local::now().timestamp_millis(),
        PENDING_DIR_SEQ.fetch_add(1, Ordering::Relaxed)
    ))
}

/// Stores under `root/<serial>` the fetched `files` whose parse or import failed, so they are kept
/// for a later look instead of being lost. A file with the same name is replaced.
pub(crate) fn store_pending_files<'a, I>(root: &Path, serial: &str, files: I)
where
    I: IntoIterator<Item = &'a FitFile>,
{
    let dir = root.join(serial);
    for file in files {
        // Names come from the device or a Connect archive, so only the last component is kept
        let Some(name) = Path::new(&file.name).file_name() else {
            error!("Invalid name for pending file '{}'", file.name);
            continue;
        };
        let dst = dir.join(name);
        let res = fs::create_dir_all(&dir).and_then(|_| fs::write(&dst, &file.data));
        match res {
            Ok(()) => warn!("Stored pending file {}", dst.display()),
            Err(e) => error!("Error storing pending file {}: {e}", dst.display()),
        }
    }
}

/// Reads an integer column from a non-mapped select row.
pub(crate) fn row_int(row: &Row, column: &str) -> i64 {
    match row.get(column) {
        Some(Value::Int64(v)) => *v,
        _ => unreachable!(),
    }
}

/// Reads a real column from a non-mapped select row.
pub(crate) fn row_f64(row: &Row, column: &str) -> f64 {
    match row.get(column) {
        Some(Value::Float64(v)) => *v,
        Some(Value::Int64(v)) => *v as f64,
        _ => unreachable!(),
    }
}

/// Reads a nullable integer column from a non-mapped select row.
pub(crate) fn row_int_nullable(row: &Row, column: &str) -> Option<i64> {
    match row.get(column) {
        Some(Value::Null) | None => None,
        _ => Some(row_int(row, column)),
    }
}

/// Reads a nullable real column from a non-mapped select row.
pub(crate) fn row_f64_nullable(row: &Row, column: &str) -> Option<f64> {
    match row.get(column) {
        Some(Value::Null) | None => None,
        _ => Some(row_f64(row, column)),
    }
}

/// Reads a text column from a non-mapped select row.
pub(crate) fn row_text(row: &Row, column: &str) -> String {
    match row.get(column) {
        Some(Value::Text(v)) => v.clone(),
        _ => unreachable!(),
    }
}

// Tauri commands: thin entry points over the logic of each module, which is what tests call

// Sessions

#[traced_command]
#[tauri::command]
pub async fn get_sessions(
    app: AppHandle,
    settings: State<'_, SettingsLock>,
    limit: Option<i32>,
) -> Result<Vec<SessionListItem>, String> {
    sessions::get_sessions(&app, &settings, limit).await
}

#[traced_command]
#[tauri::command]
pub async fn get_session_details(
    app: AppHandle,
    settings: State<'_, SettingsLock>,
    timestamp: u32,
) -> Result<SessionDetails, String> {
    sessions::get_session_details(&app, &settings, timestamp).await
}

#[traced_command]
#[tauri::command]
pub async fn download_session(
    app: AppHandle,
    settings: State<'_, SettingsLock>,
    timestamp: u32,
) -> Result<(), String> {
    sessions::download_session(&app, &settings, timestamp).await
}

#[traced_command]
#[tauri::command]
pub async fn save_session_changes(
    app: AppHandle,
    settings: State<'_, SettingsLock>,
    details: SessionStepsUpdate,
) -> Result<(), String> {
    sessions::save_session_changes(&app, &settings, details).await
}

#[traced_command]
#[tauri::command]
pub async fn import_from_files(
    app: AppHandle,
    settings: State<'_, SettingsLock>,
) -> Result<usize, String> {
    sessions::import_from_files(&app, &settings).await
}

#[traced_command]
#[tauri::command]
pub async fn get_heatmap_data(app: AppHandle) -> Result<Vec<Vec<(u32, u16)>>, String> {
    sessions::get_heatmap_data(&app).await
}

// Exercises

#[traced_command]
#[tauri::command]
pub async fn get_exercises(
    app: AppHandle,
    settings: State<'_, SettingsLock>,
) -> Result<Vec<ExerciseListItem>, String> {
    exercises::get_exercises(&app, &settings).await
}

#[traced_command]
#[tauri::command]
pub async fn get_exercise_details(
    app: AppHandle,
    settings: State<'_, SettingsLock>,
    category: u16,
    id: u16,
) -> Result<ExerciseDetails, String> {
    exercises::get_exercise_details(&app, &settings, category, id).await
}

#[traced_command]
#[tauri::command]
pub async fn get_exercises_catalog(app: AppHandle) -> Result<HashMap<u16, Vec<u16>>, String> {
    exercises::get_exercises_catalog(&app).await
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
    exercises::set_exercise_unilateral(&app, &settings, category, id, value).await
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
    exercises::set_exercise_progression(&app, &settings, category, id, value).await
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
    exercises::set_exercise_progression_parameters(&app, &settings, category, id, reps, weight)
        .await
}

#[traced_command]
#[tauri::command]
pub async fn get_progression_status(
    app: AppHandle,
    category: u16,
    id: u16,
) -> Result<Option<ProgressionSuggestion>, String> {
    exercises::get_progression_status(&app, category, id).await
}

#[traced_command]
#[tauri::command]
pub async fn get_progression_suggestion(
    app: AppHandle,
    category: u16,
    id: u16,
) -> Result<Option<ProgressionSuggestion>, String> {
    exercises::get_progression_suggestion(&app, category, id).await
}

#[traced_command]
#[tauri::command]
pub async fn available_any_progression_suggestions(app: AppHandle) -> Result<bool, String> {
    Ok(exercises::available_any_progression_suggestions(&app).await)
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
    exercises::apply_progression_suggestion(&app, &settings, &connect, category, id, suggestion)
        .await
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
    exercises::save_exercise_notes(&app, &settings, &connect, category, id, notes).await
}

// Workouts

#[traced_command]
#[tauri::command]
pub async fn get_workout_list(
    app: AppHandle,
    settings: State<'_, SettingsLock>,
) -> Result<Vec<WorkoutListItem>, String> {
    workouts::get_workout_list(&app, &settings).await
}

#[traced_command]
#[tauri::command]
pub async fn get_workout_details(
    app: AppHandle,
    settings: State<'_, SettingsLock>,
    name: &str,
) -> Result<WorkoutDetails, String> {
    workouts::get_workout_details(&app, &settings, name).await
}

#[traced_command]
#[tauri::command]
pub async fn set_workout_status(app: AppHandle, workout: &str, status: bool) -> Result<(), String> {
    workouts::set_workout_status(&app, workout, status).await
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
    workouts::save_workout(&app, &settings, &connect, edit, workout).await
}

#[traced_command]
#[tauri::command]
pub async fn load_workout_page(app: AppHandle, workout: &str) -> Result<(), String> {
    workouts::load_workout_page(&app, workout).await
}

#[traced_command]
#[tauri::command]
pub async fn workout_print_ready(window: WebviewWindow, workout: &str) -> Result<(), String> {
    workouts::workout_print_ready(window, workout).await
}

// Devices

#[traced_command]
#[tauri::command]
pub async fn send_to_device(
    app: AppHandle,
    settings: State<'_, SettingsLock>,
    workout: &str,
    serial: &str,
) -> Result<(), String> {
    devices::send_to_device(&app, &settings, workout, serial).await
}

#[traced_command]
#[tauri::command]
pub async fn import_from_device(app: AppHandle, serial: &str) -> Result<usize, String> {
    devices::_import_from_device(&app, serial).await
}

#[traced_command]
#[tauri::command]
pub async fn get_registered_devices(app: AppHandle) -> Result<Vec<DeviceListItem>, String> {
    devices::get_registered_devices(&app).await
}

// App

#[traced_command]
#[tauri::command]
pub async fn get_settings(settings: State<'_, SettingsLock>) -> Result<Settings, String> {
    app::get_settings(&settings).await
}

#[traced_command]
#[tauri::command]
pub async fn notify_frontend_ready(app: AppHandle) {
    app::notify_frontend_ready(&app).await
}

#[traced_command]
#[tauri::command]
pub async fn get_environment() -> AppEnvironment {
    app::get_environment().await
}

#[traced_command]
#[tauri::command]
pub async fn update_settings_value(app: AppHandle, name: &str, value: &str) -> Result<(), String> {
    app::update_settings_value(&app, name, value).await
}

#[traced_command(log_payload = false)]
#[tauri::command]
pub async fn get_translations(
    settings: State<'_, SettingsLock>,
) -> Result<HashMap<String, String>, String> {
    app::get_translations(&settings).await
}

// Report

#[traced_command]
#[tauri::command]
pub async fn load_report(app: AppHandle, from: u32, to: u32) -> Result<(), String> {
    report::load_report(&app, from, to).await
}

#[traced_command]
#[tauri::command]
pub async fn report_ready(window: WebviewWindow) -> Result<(), String> {
    report::report_ready(window).await
}

#[traced_command(log_payload = false)]
#[tauri::command]
pub async fn get_report_data(
    app: AppHandle,
    settings: State<'_, SettingsLock>,
    from: u32,
    to: u32,
) -> Result<ReportData, String> {
    report::get_report_data(&app, &settings, from, to).await
}

// Body metrics

#[traced_command]
#[tauri::command]
pub async fn get_body_measures(
    app: AppHandle,
    settings: State<'_, SettingsLock>,
) -> Result<Vec<BodyMetricListItem>, String> {
    body_metrics::get_body_measures(&app, &settings).await
}

#[traced_command]
#[tauri::command]
pub async fn add_body_measures(
    app: AppHandle,
    settings: State<'_, SettingsLock>,
    measures: BodyMetricListItem,
) -> Result<(), String> {
    body_metrics::add_body_measures(&app, &settings, measures).await
}

#[traced_command]
#[tauri::command]
pub async fn delete_body_metric(
    app: AppHandle,
    settings: State<'_, SettingsLock>,
    date: u32,
) -> Result<(), String> {
    body_metrics::delete_body_metric(&app, &settings, date).await
}

// Export

#[traced_command]
#[tauri::command]
pub async fn upload_to_cloud(
    settings: State<'_, SettingsLock>,
    provider: CloudProvider,
) -> Result<(), String> {
    export::upload_to_cloud(&settings, provider).await
}

#[traced_command]
#[tauri::command]
pub async fn export_database(
    app: AppHandle,
    settings: State<'_, SettingsLock>,
) -> Result<(), String> {
    export::export_database(&app, &settings).await
}

// Connect

#[traced_command]
#[tauri::command]
pub async fn import_from_connect(
    app: AppHandle,
    settings: State<'_, SettingsLock>,
    connect: State<'_, ConnectLock>,
) -> Result<usize, String> {
    connect::import_from_connect(&app, &settings, &connect).await
}

#[traced_command]
#[tauri::command]
pub async fn import_weight_from_connect(
    app: AppHandle,
    settings: State<'_, SettingsLock>,
    connect: State<'_, ConnectLock>,
) -> Result<usize, String> {
    connect::import_weight_from_connect(&app, &settings, &connect).await
}

#[traced_command]
#[tauri::command]
pub async fn connect_logged_user(
    connect: State<'_, ConnectLock>,
) -> Result<Option<String>, String> {
    connect::connect_logged_user(&connect).await
}

#[traced_command]
#[tauri::command]
pub async fn connect_login(
    app: AppHandle,
    connect: State<'_, ConnectLock>,
) -> Result<Option<String>, String> {
    connect::connect_login(&app, &connect).await
}

#[traced_command]
#[tauri::command]
pub async fn connect_logout(connect: State<'_, ConnectLock>) -> Result<(), String> {
    connect::connect_logout(&connect).await
}

#[traced_command]
#[tauri::command]
pub async fn send_to_connect(
    app: AppHandle,
    settings: State<'_, SettingsLock>,
    workout: &str,
    serial: &str,
) -> Result<(), String> {
    connect::send_to_connect(&app, &settings, workout, serial).await
}

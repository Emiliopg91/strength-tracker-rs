pub mod commands;
pub mod helpers;

use std::collections::{HashMap, HashSet};

use rusqlite_orm::errors::DatabaseError;
use tauri::{AppHandle, Runtime};
use tauri_plugin_log::log::info;

use self::helpers::{
    any_progression_suggestion, exercise_list, exercises_catalog, fetch_exercise_details,
    fetch_progression_suggestion,
};
use crate::{
    ConnectLock, SettingsLock,
    dao::{exercise::Exercise, workout::Workout, workout_step::WorkoutStep},
    dto::exercises::{ExerciseDetails, ExerciseListItem, ProgressionSuggestion},
    logic::{
        connect::helpers::sync_workouts_connect, devices::helpers::push_pending_to_usb, pool,
        report_error, sessions::helpers::spawn_percent_load_sync,
    },
    utils::time::now,
};

/// Returns the exercises with a personal record, annotated with it, plus those planned in some
/// workout step that have none yet.
pub async fn get_exercises<R: Runtime>(
    app: &AppHandle<R>,
    settings: &SettingsLock,
) -> Result<Vec<ExerciseListItem>, String> {
    info!("Getting exercises list...");
    pool(app)
        .run_in_connection(async move |conn| Ok(exercise_list(conn).await?))
        .await
        .inspect(|l| info!("Retreived {} exercises", l.len()))
        .map_err(|e| {
            report_error(
                e,
                settings,
                "error_exercise_list",
                "Error getting exercises list",
            )
        })
}

/// Returns the personal record and full per-session set history for one exercise.
pub async fn get_exercise_details<R: Runtime>(
    app: &AppHandle<R>,
    settings: &SettingsLock,
    category: u16,
    id: u16,
) -> Result<ExerciseDetails, String> {
    info!(
        "Getting details for exercise with category {} and id {}...",
        category, id
    );
    pool(app)
        .run_in_connection(async move |conn| {
            Ok(fetch_exercise_details(conn, category, id, None).await?)
        })
        .await
        .inspect(|_| info!("Found details for exercise {} - {}", category, id))
        .map_err(|e| {
            report_error(
                e,
                settings,
                "error_exercise_details",
                "Error getting exercise details",
            )
        })
}

pub async fn get_exercises_catalog<R: Runtime>(
    app: &AppHandle<R>,
) -> Result<HashMap<u16, Vec<u16>>, String> {
    pool(app)
        .run_in_connection(async move |conn| Ok(exercises_catalog(conn).await?))
        .await
        .map_err(|e: DatabaseError| e.to_string())
}

pub async fn set_exercise_unilateral<R: Runtime>(
    app: &AppHandle<R>,
    settings: &SettingsLock,
    category: u16,
    id: u16,
    value: bool,
) -> Result<(), String> {
    pool(app)
        .run_in_transaction(async move |tx| {
            Exercise::set_unilateral(tx, category, id, value).await?;
            // Unilateral sets estimate the 1RM from half their reps
            let keys: HashSet<(u16, u16)> = [(category, id)].into();
            Ok(WorkoutStep::refresh_percent_loads(tx, Some(&keys), now()).await?)
        })
        .await
        .map(|refreshed| spawn_percent_load_sync(app, refreshed))
        .inspect(|_| info!("Updated unilateral flag for exercise"))
        .map_err(|e| {
            report_error(
                e,
                settings,
                "error_updating_exercise",
                "Error updating unilateral flag for exercise",
            )
        })
}

pub async fn set_exercise_progression<R: Runtime>(
    app: &AppHandle<R>,
    settings: &SettingsLock,
    category: u16,
    id: u16,
    value: bool,
) -> Result<(), String> {
    pool(app)
        .run_in_connection(async move |conn| {
            Ok(Exercise::set_progression(conn, category, id, value).await?)
        })
        .await
        .inspect(|_| info!("Updated progression flag for exercise"))
        .map_err(|e| {
            report_error(
                e,
                settings,
                "error_updating_exercise",
                "Error updating progression flag for exercise",
            )
        })
}

pub async fn set_exercise_progression_parameters<R: Runtime>(
    app: &AppHandle<R>,
    settings: &SettingsLock,
    category: u16,
    id: u16,
    reps: &str,
    weight: f32,
) -> Result<(), String> {
    let reps = reps.to_string();
    pool(app)
        .run_in_transaction(async move |tx| {
            Exercise::set_progression_parameters(tx, category, id, reps, weight).await?;
            // Percentage loads are rounded to the weight increment
            let keys: HashSet<(u16, u16)> = [(category, id)].into();
            Ok(WorkoutStep::refresh_percent_loads(tx, Some(&keys), now()).await?)
        })
        .await
        .map(|refreshed| spawn_percent_load_sync(app, refreshed))
        .inspect(|_| info!("Updated rep ranges for exercise"))
        .map_err(|e| {
            report_error(
                e,
                settings,
                "error_updating_exercise",
                "Error updating rep ranges for exercise",
            )
        })
}

pub async fn get_progression_status<R: Runtime>(
    app: &AppHandle<R>,
    category: u16,
    id: u16,
) -> Result<Option<ProgressionSuggestion>, String> {
    pool(app)
        .run_in_connection(async move |conn| {
            let workouts = Workout::with_progression(conn).await?;
            Ok(WorkoutStep::get_highest_load(conn, category, id, &workouts).await?)
        })
        .await
        .map(|load| load.map(|(weight, reps)| ProgressionSuggestion { weight, reps }))
        .map_err(|e: DatabaseError| e.to_string())
}

pub async fn get_progression_suggestion<R: Runtime>(
    app: &AppHandle<R>,
    category: u16,
    id: u16,
) -> Result<Option<ProgressionSuggestion>, String> {
    pool(app)
        .run_in_connection(async move |conn| {
            Ok(fetch_progression_suggestion(conn, category, id).await?)
        })
        .await
        .map_err(|e: DatabaseError| e.to_string())
}

pub async fn apply_progression_suggestion<R: Runtime>(
    app: &AppHandle<R>,
    settings: &SettingsLock,
    connect: &ConnectLock,
    category: u16,
    id: u16,
    suggestion: ProgressionSuggestion,
) -> Result<(), String> {
    let res =
        pool(app)
            .run_in_transaction(async move |tx| {
                Ok(WorkoutStep::apply_progression(
                    tx,
                    category,
                    id,
                    suggestion.weight,
                    suggestion.reps,
                )
                .await?)
            })
            .await
            .map_err(|e| e.to_string());

    match res {
        Ok((modified, workouts)) => {
            info!("Applied suggested progression");
            sync_workouts_connect(app, connect, settings, &workouts).await;
            if modified {
                push_pending_to_usb(app).await;
            }
            Ok(())
        }
        Err(e) => Err(report_error(
            e,
            settings,
            "error_applying_progression",
            "Error applying progression",
        )),
    }
}

pub async fn save_exercise_notes<R: Runtime>(
    app: &AppHandle<R>,
    settings: &SettingsLock,
    connect: &ConnectLock,
    category: u16,
    id: u16,
    notes: &str,
) -> Result<(), String> {
    let notes = if notes.trim().is_empty() {
        None
    } else {
        Some(notes.trim().to_string())
    };

    let workouts = pool(app)
        .run_in_transaction(async move |tx| {
            Ok(Exercise::update_notes(tx, category, id, notes.clone()).await?)
        })
        .await
        .map_err(|e| e.to_string())?;

    if !workouts.is_empty() {
        sync_workouts_connect(app, connect, settings, &workouts).await;
        push_pending_to_usb(app).await;
    }

    Ok(())
}

pub async fn available_any_progression_suggestions(app: &AppHandle) -> bool {
    pool(app)
        .run_in_connection(async move |conn| Ok(any_progression_suggestion(conn).await?))
        .await
        .unwrap_or(false)
}

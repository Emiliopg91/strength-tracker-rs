pub mod commands;
pub mod queries;

use std::collections::HashMap;

use rusqlite_orm::errors::DatabaseError;
use tauri::{AppHandle, Runtime};
use tauri_plugin_log::log::info;

use self::queries::{
    any_progression_suggestion, apply_progression, exercise_list, exercises_catalog,
    fetch_exercise_details, fetch_progression_suggestion, progression_status, set_progression,
    set_progression_parameters, set_unilateral, update_exercise_notes,
};
use crate::{
    ConnectLock, SettingsLock,
    dto::exercises::{ExerciseDetails, ExerciseListItem, ProgressionSuggestion},
    logic::{
        connect::helpers::sync_workouts_connect, devices::helpers::push_pending_to_usb,
        report_error, run_blocking, run_blocking_conn, run_blocking_tx,
    },
};

/// Returns the exercises with a personal record, annotated with it, plus those planned in some
/// workout step that have none yet.
pub async fn get_exercises<R: Runtime>(
    app: &AppHandle<R>,
    settings: &SettingsLock,
) -> Result<Vec<ExerciseListItem>, String> {
    info!("Getting exercises list...");
    let res = run_blocking_conn(app, move |_, conn| exercise_list(conn)).await;

    match res {
        Ok(l) => {
            info!("Retreived {} exercises", l.len());
            Ok(l)
        }
        Err(e) => Err(report_error(
            e,
            settings.read().unwrap().language,
            "error_exercise_list",
            "Error getting exercises list",
        )),
    }
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
    let res = run_blocking_conn(app, move |_, conn| {
        fetch_exercise_details(conn, category, id, None)
    })
    .await;
    match res {
        Ok(l) => {
            info!("Found details for exercise {} - {}", category, id);
            Ok(l)
        }
        Err(e) => Err(report_error(
            e,
            settings.read().unwrap().language,
            "error_exercise_details",
            "Error getting exercise details",
        )),
    }
}

pub async fn get_exercises_catalog<R: Runtime>(
    app: &AppHandle<R>,
) -> Result<HashMap<u16, Vec<u16>>, String> {
    run_blocking_conn(app, move |_, conn| exercises_catalog(conn))
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
    let res = run_blocking(app, move |database| {
        set_unilateral(database, category, id, value)
    })
    .await;

    match res {
        Ok(l) => {
            info!("Updated unilateral flag for exercise");
            Ok(l)
        }
        Err(e) => Err(report_error(
            e,
            settings.read().unwrap().language,
            "error_updating_exercise",
            "Error updating unilateral flag for exercise",
        )),
    }
}

pub async fn set_exercise_progression<R: Runtime>(
    app: &AppHandle<R>,
    settings: &SettingsLock,
    category: u16,
    id: u16,
    value: bool,
) -> Result<(), String> {
    let res = run_blocking(app, move |database| {
        set_progression(database, category, id, value)
    })
    .await;

    match res {
        Ok(l) => {
            info!("Updated progression flag for exercise");
            Ok(l)
        }
        Err(e) => Err(report_error(
            e,
            settings.read().unwrap().language,
            "error_updating_exercise",
            "Error updating progression flag for exercise",
        )),
    }
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
    let res = run_blocking(app, move |database| {
        set_progression_parameters(database, category, id, reps, weight)
    })
    .await;

    match res {
        Ok(l) => {
            info!("Updated rep ranges for exercise");
            Ok(l)
        }
        Err(e) => Err(report_error(
            e,
            settings.read().unwrap().language,
            "error_updating_exercise",
            "Error updating rep ranges for exercise",
        )),
    }
}

pub async fn get_progression_status<R: Runtime>(
    app: &AppHandle<R>,
    category: u16,
    id: u16,
) -> Result<Option<ProgressionSuggestion>, String> {
    run_blocking(app, move |database| {
        progression_status(database, category, id)
    })
    .await
    .map_err(|e: DatabaseError| e.to_string())
}

pub async fn get_progression_suggestion<R: Runtime>(
    app: &AppHandle<R>,
    category: u16,
    id: u16,
) -> Result<Option<ProgressionSuggestion>, String> {
    run_blocking_conn(app, move |_, conn| {
        fetch_progression_suggestion(conn, category, id)
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
    let res = run_blocking_tx(app, move |_, tx| {
        apply_progression(tx, category, id, &suggestion)
    })
    .await
    .map_err(|e| e.to_string());

    let lang = settings.read().unwrap().language;
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
            lang,
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

    let workouts = run_blocking_tx(app, move |_, tx| {
        update_exercise_notes(tx, category, id, notes.clone())
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
    run_blocking_conn(app, move |_, conn| any_progression_suggestion(conn))
        .await
        .unwrap_or(false)
}

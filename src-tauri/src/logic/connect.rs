use std::collections::BTreeSet;

use chrono::Local;
use futures_util::TryFutureExt;
use rusqlite_orm::{
    dao::Repository, database::DatabasePool, errors::DatabaseError, types::where_clause::Where,
};
use strength_tracker_rs_macros::traced_command;
use tauri::{AppHandle, Manager, State};
use tauri_plugin_log::log::info;

use crate::{
    ConnectLock, SettingsLock,
    dao::{
        body_metric::BodyMetricRepository,
        device::{self, DeviceRepository},
        session::{self, SessionRepository},
        workout::WorkoutRepository,
    },
    dto::connect::ConnectWorkout,
    logic::{
        new_pending_dir, report_error, row_int, run_blocking,
        sessions::{import_pending_sessions, update_pending_geolocation},
    },
    utils::constants,
};

//31-aug-2026 UTC
const MIN_DATE: u32 = 1788134400;
//1 day
const SYNC_OVERLAP: u32 = 86_400;

#[traced_command]
#[tauri::command]
pub async fn import_from_connect(
    app: AppHandle,
    settings: State<'_, SettingsLock>,
    connect: State<'_, ConnectLock>,
) -> Result<usize, String> {
    let lang = settings.read().unwrap().language;
    let lang_cl = lang;

    let app_cpy = app.clone();
    let res: Result<_, String> = async {
        let (mut device, from_date, existing) = run_blocking(&app, |database| {
            let device = DeviceRepository::select()
                .where_(Where::Eq(device::entity::columns::SERIAL, "connect".into()))
                .fetch_one(database)?
                .unwrap();

            // Overlap covers activities uploaded out of order; those already stored are skipped
            let mark = device.last_activity_sync.unwrap_or(MIN_DATE);
            let from_date = mark.saturating_sub(SYNC_OVERLAP).max(MIN_DATE);

            let existing: BTreeSet<u32> = database.run_in_connection(|conn| {
                Ok(SessionRepository::select()
                    .columns(&[session::entity::columns::DATE])
                    .where_(Where::Gte(session::entity::columns::DATE, from_date.into()))
                    .fetch_in(conn)?
                    .iter()
                    .map(|row| row_int(row, session::entity::columns::DATE.as_ref()) as u32)
                    .collect())
            })?;

            Ok((device, from_date, existing))
        })
        .map_err(|e: DatabaseError| e.to_string())
        .await?;

        let dst_dir = new_pending_dir(&constants::PENDING_DOWNLOAD_SESSIONS_FOLDER, &device.serial);
        tokio::fs::create_dir_all(&dst_dir)
            .await
            .map_err(|e| e.to_string())?;

        let newest = connect
            .lock()
            .await
            .download_activities(from_date, &existing, &dst_dir)
            .await
            .map_err(|e| e.to_string())?;

        // The mark only follows what Connect has listed, so activities uploaded late are not missed
        let sync_mark = newest.filter(|&newest| Some(newest) > device.last_activity_sync);
        let imported_sessions = tokio::task::spawn_blocking(move || {
            let db = app_cpy.state::<DatabasePool>();
            import_pending_sessions(&db, &dst_dir, &mut device, lang_cl, sync_mark)
        })
        .await
        .expect("blocking DB task panicked")
        .map_err(|e: DatabaseError| e.to_string())?;

        Ok(imported_sessions)
    }
    .await;

    match res {
        Ok(res) => {
            if !res.is_empty() {
                let app = app.clone();
                std::thread::spawn(move || {
                    let db = app.state::<DatabasePool>();
                    update_pending_geolocation(&app, &db);
                });
            }
            Ok(res.len())
        }
        Err(e) => Err(report_error(
            e,
            lang,
            "error_import_sessions",
            "Error importing sessions",
        )),
    }
}

#[traced_command]
#[tauri::command]
pub async fn import_weight_from_connect(
    app: AppHandle,
    settings: State<'_, SettingsLock>,
    connect: State<'_, ConnectLock>,
) -> Result<(), String> {
    let lang = settings.read().unwrap().language;

    let app_cpy = app.clone();

    let res: Result<(), String> = async {
        let mut device = run_blocking(&app, |database| {
            Ok(DeviceRepository::select()
                .where_(Where::Eq(device::entity::columns::SERIAL, "connect".into()))
                .fetch_one(database)?
                .unwrap())
        })
        .map_err(|e: DatabaseError| e.to_string())
        .await?;

        let mut entries = connect
            .lock()
            .await
            .list_weight(device.last_body_metric_sync)
            .await
            .map_err(|e| e.to_string())?;

        tokio::task::spawn_blocking(move || {
            let db = app_cpy.state::<DatabasePool>();
            db.run_in_transaction(move |tx| {
                let mut insert = BodyMetricRepository::insert().or_replace();
                for entry in &mut entries {
                    insert = insert.item(entry);
                }
                insert.execute_in(tx)?;

                device.last_body_metric_sync = Some(Local::now().timestamp() as u32);
                device.update_by_id_in(tx)?;

                Ok(())
            })
        })
        .await
        .expect("blocking DB task panicked")
        .map_err(|e: DatabaseError| e.to_string())?;

        Ok(())
    }
    .await;

    match res {
        Ok(_) => {
            info!("Measures added succesfully");
            Ok(())
        }
        Err(e) => Err(report_error(
            e,
            lang,
            "error_adding_body_measures",
            "Error adding measures",
        )),
    }
}

#[traced_command]
#[tauri::command]
pub async fn connect_logged_user(
    connect: State<'_, ConnectLock>,
) -> Result<Option<String>, String> {
    Ok(connect.lock().await.logged_user())
}

#[traced_command]
#[tauri::command]
pub async fn connect_login(connect: State<'_, ConnectLock>) -> Result<Option<String>, String> {
    let mut connect = connect.lock().await;

    connect.prompt_login().await.map_err(|e| e.to_string())?;

    Ok(connect.logged_user())
}

#[traced_command]
#[tauri::command]
pub async fn connect_logout(connect: State<'_, ConnectLock>) -> Result<(), String> {
    connect.lock().await.disconnect().map_err(|e| e.to_string())
}

pub async fn send_workout_connect(
    app: &AppHandle,
    connect: &State<'_, ConnectLock>,
    settings: &State<'_, SettingsLock>,
    workout: &str,
) -> Result<(), String> {
    let weight_unit = settings.read().unwrap().weight_unit;

    let id = workout.to_string();
    let mut workout = run_blocking(app, move |database| {
        database.run_in_connection(move |conn| {
            let Some(mut workout) = WorkoutRepository::select_by_id_in(conn, &id)? else {
                return Ok(None);
            };
            workout.fetch_steps_relationship_in(conn)?;
            workout.steps.sort_by_key(|s| s.idx);
            for step in &mut workout.steps {
                step.fetch_exercise_relationship_in(conn)?;
                if let Some(exercise) = &mut step.exercise {
                    exercise.fetch_exercise_category_relationship_in(conn)?;
                }
            }
            Ok(Some(workout))
        })
    })
    .await
    .map_err(|e| e.to_string())?
    .ok_or_else(|| format!("Workout {workout} not found"))?;

    let mut client = connect.lock().await;
    let original_id = workout.connect_id.clone();

    if workout.connect_id.is_none() {
        workout.connect_id = client
            .list_workouts()
            .await
            .map_err(|e| e.to_string())?
            .into_iter()
            .find(|w| w.workout_name == workout.name)
            .map(|w| w.workout_id.to_string());
    }

    let payload = ConnectWorkout::from(&workout, weight_unit);
    match workout.connect_id.as_deref() {
        Some(id) => {
            let id = id
                .parse::<u64>()
                .map_err(|e| format!("Invalid Connect workout id '{id}': {e}"))?;
            client
                .update_workout(id, &payload)
                .await
                .map_err(|e| e.to_string())?;
        }
        None => {
            let created = client
                .create_workout(&payload)
                .await
                .map_err(|e| e.to_string())?;
            workout.connect_id = Some(created.workout_id.to_string());
        }
    }
    drop(client);

    if workout.connect_id != original_id {
        run_blocking(app, |database| {
            database.run_in_transaction(move |tx| Ok(workout.update_by_id_in(tx)?))
        })
        .await
        .map_err(|e| e.to_string())?;
    }

    Ok(())
}

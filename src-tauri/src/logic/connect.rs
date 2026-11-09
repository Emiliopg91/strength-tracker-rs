use std::collections::{BTreeSet, HashMap};

use futures_util::TryFutureExt;
use rusqlite_orm::{
    dao::Repository,
    database::DatabasePool,
    errors::DatabaseError,
    types::{order_by::OrderBy, where_clause::Where},
};
use tauri::{AppHandle, Emitter, Manager, Runtime};
use tauri_plugin_log::log::{error, info};

use crate::{
    ConnectLock, SettingsLock,
    connect::{ConnectClient, client::CalendarItemType},
    dao::{
        body_metric::{self, BodyMetricRepository},
        device::{self, Device, DeviceRepository},
        exercise::{self, Exercise, ExerciseRepository},
        exercise_category::{self, ExerciseCategory, ExerciseCategoryRepository},
        session::{self, SessionRepository},
        workout::WorkoutRepository,
        workout_device::{self, WorkoutDevice, WorkoutDeviceRepository},
        workout_step::WorkoutStep,
    },
    dto::{
        connect::{ConnectUpdateSets, ConnectWorkout},
        devices::DeviceListItem,
        notifications::{NotificationDefinition, NotificationKind},
        sessions::SessionStepDto,
    },
    logic::{
        notifications::show_notification,
        report_error, run_blocking,
        sessions::{import_pending_sessions, update_pending_geolocation},
    },
    utils::{constants, translations::translate},
};

pub async fn import_from_connect<R: Runtime>(
    app: &AppHandle<R>,
    settings: &SettingsLock,
    connect: &ConnectLock,
) -> Result<usize, String> {
    let lang = settings.read().unwrap().language;
    let lang_cl = lang;

    let app_cpy = app.clone();
    let res: Result<_, String> = async {
        let (mut device, existing) = run_blocking(app, |database| {
            let device = DeviceRepository::select()
                .where_(Where::Eq(
                    device::entity::columns::SERIAL,
                    constants::CONNECT_DEVICE_SERIAL.into(),
                ))
                .fetch_one(database)?
                .unwrap();

            let existing: BTreeSet<u32> = SessionRepository::select()
                .columns(&[session::entity::columns::DATE])
                .fetch(database)?
                .iter()
                .map(|row| row.get_u32(session::entity::columns::DATE.as_ref()))
                .collect();

            Ok((device, existing))
        })
        .map_err(|e: DatabaseError| e.to_string())
        .await?;

        let mut connect = connect.lock().await;

        let calendar_items = connect
            .list_recent_calendar_items()
            .await
            .map_err(|e| e.to_string())?;

        let pending: Vec<u64> = calendar_items
            .iter()
            .filter_map(|e| {
                if e.item_type == CalendarItemType::Activity
                    && let Some(date) = e.start_timestamp_local
                    && !existing.contains(&date)
                {
                    e.id
                } else {
                    None
                }
            })
            .collect();

        let files = connect
            .download_activities(&pending)
            .await
            .map_err(|e| e.to_string())?;

        // The mark only follows what Connect has listed, so activities uploaded late are not missed
        let imported_sessions = tokio::task::spawn_blocking(move || {
            let db = app_cpy.state::<DatabasePool>();
            import_pending_sessions(&app_cpy, &db, &files, &mut device, lang_cl, None)
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

pub async fn import_weight_from_connect<R: Runtime>(
    app: &AppHandle<R>,
    settings: &SettingsLock,
    connect: &ConnectLock,
) -> Result<usize, String> {
    let lang = settings.read().unwrap().language;

    let app_cpy = app.clone();

    let res: Result<usize, String> = async {
        let existing = run_blocking(app, |database| {
            let existing: Vec<u32> = BodyMetricRepository::select()
                .columns(&[body_metric::entity::columns::DATE])
                .order_by(OrderBy::Desc(body_metric::entity::columns::DATE))
                .fetch(database)?
                .iter()
                .map(|row| row.get_u32(session::entity::columns::DATE.as_ref()))
                .collect();

            Ok(existing)
        })
        .map_err(|e: DatabaseError| e.to_string())
        .await?;

        let mut entries = connect
            .lock()
            .await
            .list_weight(existing.first().copied())
            .await
            .map_err(|e| e.to_string())?;

        tokio::task::spawn_blocking(move || {
            let db = app_cpy.state::<DatabasePool>();
            // Runs in a transaction only when several entries are inserted
            let mut insert = BodyMetricRepository::insert().or_replace();
            for entry in &mut entries {
                if !existing.contains(&entry.date) {
                    insert = insert.item(entry);
                }
            }
            insert.execute(&db)
        })
        .await
        .expect("blocking DB task panicked")
        .map_err(|e: DatabaseError| e.to_string())
    }
    .await;

    match res {
        Ok(0) => {
            info!("No measures added");
            Ok(0)
        }
        Ok(added) => {
            info!("Measures {} added succesfully", added);
            Ok(added)
        }
        Err(e) => Err(report_error(
            e,
            lang,
            "error_adding_body_measures",
            "Error adding measures",
        )),
    }
}

pub async fn connect_logged_user(connect: &ConnectLock) -> Result<Option<String>, String> {
    Ok(connect.lock().await.logged_user())
}

pub async fn connect_login<R: Runtime>(
    app: &AppHandle<R>,
    connect: &ConnectLock,
) -> Result<Option<String>, String> {
    let user = {
        let mut connect = connect.lock().await;
        connect.prompt_login().await.map_err(|e| e.to_string())?;
        connect.logged_user()
    };

    // Released the lock first: the discovery needs it
    if user.is_some() {
        get_devices_from_connect(app).await;
        up_to_date_connect(app).await;
    }

    Ok(user)
}

pub async fn connect_logout(connect: &ConnectLock) -> Result<(), String> {
    connect.lock().await.disconnect().map_err(|e| e.to_string())
}

/// Uploads `workout` to Connect, notifying the user if it fails.
/// The workout is left pending of upload on failure and cleared on success.
pub async fn send_workout_connect<R: Runtime>(
    app: &AppHandle<R>,
    connect: &ConnectLock,
    settings: &SettingsLock,
    workout: &str,
) -> Result<(), String> {
    let lang = settings.read().unwrap().language;
    let res = upload_workout_connect(app, connect, settings, workout).await;
    mark_connect_pending(app, &[workout.to_string()], res.is_err()).await;
    res.map_err(|e| {
        report_error(
            format!("{workout}: {e}"),
            lang,
            "error_workout_send_connect",
            "Error sending workout to Garmin Connect",
        )
    })
}

/// Uploads the just modified `workouts` to Connect, or leaves them pending of upload when no
/// user is logged in.
pub async fn sync_workouts_connect<R: Runtime>(
    app: &AppHandle<R>,
    connect: &ConnectLock,
    settings: &SettingsLock,
    workouts: &[String],
) {
    if workouts.is_empty() {
        return;
    }

    if connect.lock().await.logged_user().is_some() {
        for workout in workouts {
            let _ = send_workout_connect(app, connect, settings, workout).await;
        }
    } else {
        mark_connect_pending(app, workouts, true).await;
    }
}

/// Retries the upload of the workouts left pending for Connect, if a user is logged in.
/// Failures are only logged, as they stay pending for the next attempt.
pub async fn up_to_date_connect<R: Runtime>(app: &AppHandle<R>) {
    let connect = app.state::<ConnectLock>();
    if connect.lock().await.logged_user().is_none() {
        return;
    }

    let pending = match run_blocking(app, |database| {
        database.run_in_connection(|conn| Ok(pending_connect_workouts(conn)?))
    })
    .await
    {
        Ok(pending) => pending,
        Err(e) => {
            error!("Error fetching workouts pending of upload to Connect: {e}");
            return;
        }
    };
    if pending.is_empty() {
        return;
    }

    info!("Uploading {} pending workouts to Connect", pending.len());
    let settings = app.state::<SettingsLock>();
    for workout in pending {
        let res = upload_workout_connect(app, &connect, &settings, &workout).await;
        if let Err(e) = &res {
            error!("Error uploading pending workout '{workout}' to Connect: {e}");
        }
        mark_connect_pending(app, std::slice::from_ref(&workout), res.is_err()).await;
    }
}

/// Workouts that still have to be uploaded to Connect.
pub fn pending_connect_workouts(
    conn: &rusqlite_orm::rusqlite::Connection,
) -> rusqlite_orm::errors::Result<Vec<String>> {
    Ok(WorkoutDeviceRepository::select()
        .where_(Where::And(vec![
            Where::Eq(
                workout_device::entity::columns::DEVICE,
                constants::CONNECT_DEVICE_SERIAL.into(),
            ),
            Where::Eq(workout_device::entity::columns::PENDING, true.into()),
        ]))
        .order_by(OrderBy::Asc(workout_device::entity::columns::WORKOUT))
        .fetch_in(conn)?
        .into_iter()
        .map(|e| e.workout)
        .collect())
}

async fn mark_connect_pending<R: Runtime>(app: &AppHandle<R>, workouts: &[String], pending: bool) {
    let workouts = workouts.to_vec();
    let res = run_blocking(app, move |database| {
        database.run_in_connection(|conn| Ok(set_connect_pending(conn, &workouts, pending)?))
    })
    .await;

    if let Err(e) = res {
        error!("Error updating Connect upload status: {e}");
    }
}

/// Records in `workout_device` whether each of `workouts` still has to be uploaded to Connect.
/// The upserts are idempotent, so a partial run (several statements for a huge list) is harmless.
pub fn set_connect_pending(
    conn: &rusqlite_orm::rusqlite::Connection,
    workouts: &[String],
    pending: bool,
) -> rusqlite_orm::errors::Result<()> {
    let mut insert = WorkoutDeviceRepository::insert().or_replace();
    let mut entries: Vec<WorkoutDevice> = workouts
        .iter()
        .map(|workout| WorkoutDevice {
            workout: workout.clone(),
            device: constants::CONNECT_DEVICE_SERIAL.to_string(),
            pending,
        })
        .collect();
    for entry in &mut entries {
        insert = insert.item(entry);
    }
    insert.execute_in(conn)?;
    Ok(())
}

async fn upload_workout_connect<R: Runtime>(
    app: &AppHandle<R>,
    connect: &ConnectLock,
    settings: &SettingsLock,
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
            fetch_step_exercises(conn, &mut workout.steps)?;
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
        run_blocking(app, move |database| workout.update_by_id(database))
            .await
            .map_err(|e| e.to_string())?;
    }

    Ok(())
}

/// Fills the exercise, with its category, of every exercise step in two queries, instead of
/// two per step as the relationship fetchers would.
pub fn fetch_step_exercises(
    conn: &rusqlite_orm::rusqlite::Connection,
    steps: &mut [WorkoutStep],
) -> rusqlite_orm::errors::Result<()> {
    let keys: BTreeSet<(u16, u16)> = steps.iter().filter_map(|s| s.ex_cat.zip(s.ex_id)).collect();
    if keys.is_empty() {
        return Ok(());
    }
    let exercises = fetch_exercises_with_category(conn, &keys)?;

    for step in steps {
        step.exercise = step
            .ex_cat
            .zip(step.ex_id)
            .and_then(|key| exercises.get(&key).cloned());
    }
    Ok(())
}

/// Loads the exercises with the given `(category, id)` keys, with their category filled, in two queries.
fn fetch_exercises_with_category(
    conn: &rusqlite_orm::rusqlite::Connection,
    keys: &BTreeSet<(u16, u16)>,
) -> rusqlite_orm::errors::Result<HashMap<(u16, u16), Exercise>> {
    let mut exercises: HashMap<(u16, u16), Exercise> = ExerciseRepository::select()
        .where_(Where::InMultiple(
            vec![
                exercise::entity::columns::CATEGORY,
                exercise::entity::columns::ID,
            ],
            keys.iter()
                .map(|(cat, id)| vec![(*cat).into(), (*id).into()])
                .collect(),
        ))
        .fetch_in(conn)?
        .into_iter()
        .map(|e| ((e.category, e.id), e))
        .collect();

    let category_ids: BTreeSet<u16> = keys.iter().map(|(cat, _)| *cat).collect();
    let categories: HashMap<u16, ExerciseCategory> = ExerciseCategoryRepository::select()
        .where_(Where::In(
            exercise_category::entity::columns::ID,
            category_ids.iter().map(|id| (*id).into()).collect(),
        ))
        .fetch_in(conn)?
        .into_iter()
        .map(|c| (c.id, c))
        .collect();
    for exercise in exercises.values_mut() {
        exercise.exercise_category = categories.get(&exercise.category).cloned();
    }
    Ok(exercises)
}

/// Mirrors the workout-capable devices of the Connect account into the database: enrolls new
/// ones, refreshes the Connect-owned fields of known ones and clears the `connect` flag of those
/// no longer listed. Emits `device_discovered` per added or changed device
/// Does nothing if the device list cannot be fetched.
pub async fn get_devices_from_connect<R: Runtime>(app_orig: &AppHandle<R>) {
    let connect = app_orig.state::<ConnectLock>();
    let mut client = connect.lock().await;
    if client.logged_user().is_none() {
        return;
    }

    let devices = match client.list_devices().await {
        Ok(devices) => devices,
        Err(e) => {
            error!("Error listing Connect devices: {e}");
            return;
        }
    };
    drop(client);

    let listed: Vec<Device> = devices
        .iter()
        .filter(|d| d.workout_capable)
        .map(Device::from)
        .collect();

    let app = app_orig.clone();
    let res = run_blocking(app_orig, move |database| {
        database.run_in_transaction(move |tx| {
            for mut dao in listed.iter().cloned() {
                match DeviceRepository::select_by_id_in(tx, &dao.serial)? {
                    None => {
                        DeviceRepository::insert().item(&mut dao).execute_in(tx)?;
                    }
                    Some(db)
                        if !db.connect
                            || db.model != dao.model
                            || db.strength_workout != dao.strength_workout =>
                    {
                        DeviceRepository::update()
                            .set(device::entity::columns::CONNECT, true.into())
                            .set(device::entity::columns::MODEL, dao.model.clone().into())
                            .set(
                                device::entity::columns::STRENGTH_WORKOUT,
                                dao.strength_workout.into(),
                            )
                            .where_(Where::Eq(
                                device::entity::columns::SERIAL,
                                dao.serial.clone().into(),
                            ))
                            .execute_in(tx)?;
                    }
                    Some(_) => {}
                }

                let payload: DeviceListItem = (&dao).into();
                let _ = app.emit("device_discovered", payload);
            }

            let removed: Vec<Device> = DeviceRepository::select()
                .where_(Where::Eq(device::entity::columns::CONNECT, true.into()))
                .fetch_in(tx)?
                .into_iter()
                .filter(|db| !listed.iter().any(|d| d.serial == db.serial))
                .collect();

            if !removed.is_empty() {
                DeviceRepository::update()
                    .set(device::entity::columns::CONNECT, false.into())
                    .where_(Where::In(
                        device::entity::columns::SERIAL,
                        removed.iter().map(|d| d.serial.clone().into()).collect(),
                    ))
                    .execute_in(tx)?;
            }

            Ok(())
        })
    })
    .await;

    match res {
        Ok(()) => {}
        Err(e) => error!("Error syncing Connect devices: {e}"),
    }
}

/// Queues `workout` in Connect for delivery to the device `serial`, first uploading the workout
/// to Connect if it has no Connect id yet.
pub async fn _send_to_connect<R: Runtime>(
    app: &AppHandle<R>,
    workout: &str,
    serial: &str,
) -> Result<(), String> {
    let fetch_connect_id = || {
        let name = workout.to_string();
        run_blocking(app, move |database| {
            Ok::<_, DatabaseError>(
                WorkoutRepository::select_by_id(database, &name)?.and_then(|w| w.connect_id),
            )
        })
    };

    let connect_id = match fetch_connect_id().await.map_err(|e| e.to_string())? {
        Some(id) => id,
        None => {
            send_workout_connect(
                app,
                &app.state::<ConnectLock>(),
                &app.state::<SettingsLock>(),
                workout,
            )
            .await?;
            fetch_connect_id()
                .await
                .map_err(|e| e.to_string())?
                .ok_or_else(|| format!("Workout {workout} has no Connect id"))?
        }
    };
    let workout_id = connect_id
        .parse::<u64>()
        .map_err(|e| format!("Invalid Connect workout id '{connect_id}': {e}"))?;
    let unit_id = Device::unit_id_from_serial(serial).map_err(|e| e.to_string())?;

    app.state::<ConnectLock>()
        .lock()
        .await
        .send_workout_to_device(workout_id, workout, unit_id)
        .await
        .map_err(|e| e.to_string())
}

pub async fn send_to_connect<R: Runtime>(
    app: &AppHandle<R>,
    settings: &SettingsLock,
    workout: &str,
    serial: &str,
) -> Result<(), String> {
    let res: Result<_, Box<dyn std::error::Error>> = async {
        _send_to_connect(app, workout, serial).await?;

        // Register the pair so later changes (e.g. progressions) get re-sent once plugged by USB
        let mut entry = WorkoutDevice {
            device: serial.to_string(),
            workout: workout.to_string(),
            pending: false,
        };
        run_blocking(app, move |database| {
            WorkoutDeviceRepository::insert()
                .or_replace()
                .item(&mut entry)
                .execute(database)
        })
        .await?;

        Ok(())
    }
    .await;

    let lang = settings.read().unwrap().language;

    match res {
        Ok(()) => {
            info!("Workout '{}' sent to device {}", workout, serial);
            show_notification(NotificationDefinition {
                title: workout.to_string(),
                body: translate("ok_workout_send", lang),
                kind: NotificationKind::Temporal,
            });
            Ok(())
        }
        Err(e) => Err(report_error(
            e,
            lang,
            "error_workout_send",
            "Error sending workout to device",
        )),
    }
}

/// Mirrors the user edits of a session's sets on its Connect activity, if the session is linked to
/// one and a user is logged in. Runs after the local save, so failures are only logged.
pub async fn sync_session_sets_connect<R: Runtime>(
    app: &AppHandle<R>,
    timestamp: u32,
    sets: Vec<SessionStepDto>,
) {
    let res = run_blocking(app, move |database| {
        database.run_in_connection(|conn| {
            let Some(connect_id) = SessionRepository::select()
                .columns(&[session::entity::columns::CONNECT_ID])
                .where_(Where::Eq(session::entity::columns::DATE, timestamp.into()))
                .fetch_in(conn)?
                .first()
                .and_then(|row| {
                    row.get_text_nullable(session::entity::columns::CONNECT_ID.as_ref())
                })
            else {
                return Ok(None);
            };

            let keys: BTreeSet<(u16, u16)> =
                sets.iter().filter_map(|s| s.ex_cat.zip(s.ex_id)).collect();
            let names: HashMap<(u16, u16), (String, String)> =
                fetch_exercises_with_category(conn, &keys)?
                    .into_iter()
                    .filter_map(|(key, exercise)| {
                        Some((key, (exercise.exercise_category?.name, exercise.name)))
                    })
                    .collect();

            let updates: Vec<ConnectUpdateSets> = sets
                .iter()
                .filter_map(|set| {
                    let (exercise_category, exercise_id) =
                        names.get(&set.ex_cat.zip(set.ex_id)?)?.clone();
                    Some(ConnectUpdateSets {
                        idx: set.idx as u16,
                        reps: set.reps,
                        weight: set.weight,
                        exercise_category,
                        exercise_id,
                    })
                })
                .collect();
            Ok(Some((connect_id, updates)))
        })
    })
    .await;

    let (connect_id, updates) = match res {
        Ok(Some((connect_id, updates))) if !updates.is_empty() => (connect_id, updates),
        Ok(_) => return,
        Err(e) => {
            error!(
                "Error loading sets of session {} for Connect: {}",
                timestamp, e
            );
            return;
        }
    };
    let Ok(id) = connect_id.parse::<u64>() else {
        error!("Invalid Connect activity id '{}'", connect_id);
        return;
    };

    let connect = app.state::<ConnectLock>();
    let mut connect = connect.lock().await;
    if connect.logged_user().is_none() {
        return;
    }
    update_session_sets(&mut connect, id, updates).await;
}

/// Applies `sets` over the current sets of the Connect activity `id` and sends them back.
/// Sets are matched by their FIT message index, which is the local set index.
pub async fn update_session_sets(
    connect: &mut ConnectClient,
    id: u64,
    sets: Vec<ConnectUpdateSets>,
) {
    let Ok(mut original_sets) = connect.get_session_sets(id).await else {
        return;
    };

    for set in &sets {
        let Some(target) = original_sets
            .exercise_sets
            .iter_mut()
            .find(|s| s.message_index == Some(set.idx))
        else {
            error!("Set {} not found in Connect activity {}", set.idx, id);
            continue;
        };
        // Stored in kg, Connect expects grams
        target.weight = set.weight.map(|v| (v as f64 * 1000.0).round());
        target.repetition_count = set.reps;
        target.exercises = vec![crate::connect::client::ExerciseCandidate {
            category: set.exercise_category.clone(),
            name: Some(set.exercise_id.clone()),
            probability: 100_f64,
        }];
    }

    let _ = connect.update_session_sets(&original_sets).await;
}

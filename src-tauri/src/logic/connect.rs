use std::collections::BTreeSet;

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
    connect::client::CalendarItemType,
    dao::{
        body_metric::{self, BodyMetricRepository},
        device::{self, Device, DeviceRepository},
        session::{self, SessionRepository},
        workout::WorkoutRepository,
        workout_device::{WorkoutDevice, WorkoutDeviceRepository},
    },
    dto::{
        connect::ConnectWorkout,
        devices::DeviceListItem,
        notifications::{NotificationDefinition, NotificationKind},
    },
    logic::{
        notifications::show_notification,
        report_error, row_int, run_blocking,
        sessions::{import_pending_sessions, update_pending_geolocation},
    },
    utils::translations::translate,
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
                .where_(Where::Eq(device::entity::columns::SERIAL, "connect".into()))
                .fetch_one(database)?
                .unwrap();

            let existing: BTreeSet<u32> = database.run_in_connection(|conn| {
                Ok(SessionRepository::select()
                    .columns(&[session::entity::columns::DATE])
                    .fetch_in(conn)?
                    .iter()
                    .map(|row| row_int(row, session::entity::columns::DATE.as_ref()) as u32)
                    .collect())
            })?;

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
            let existing: Vec<u32> = database.run_in_connection(|conn| {
                Ok(BodyMetricRepository::select()
                    .columns(&[body_metric::entity::columns::DATE])
                    .order_by(OrderBy::Desc(body_metric::entity::columns::DATE))
                    .fetch_in(conn)?
                    .iter()
                    .map(|row| row_int(row, session::entity::columns::DATE.as_ref()) as u32)
                    .collect())
            })?;

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
            db.run_in_transaction(move |tx| {
                let mut insert = BodyMetricRepository::insert().or_replace();
                let mut any = false;
                for entry in &mut entries {
                    if !existing.contains(&entry.date) {
                        any = true;
                        insert = insert.item(entry);
                    }
                }
                if any {
                    Ok(insert.execute_in(tx)?)
                } else {
                    Ok(0)
                }
            })
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
    }

    Ok(user)
}

pub async fn connect_logout(connect: &ConnectLock) -> Result<(), String> {
    connect.lock().await.disconnect().map_err(|e| e.to_string())
}

/// Uploads `workout` to Connect, notifying the user if it fails.
pub async fn send_workout_connect<R: Runtime>(
    app: &AppHandle<R>,
    connect: &ConnectLock,
    settings: &SettingsLock,
    workout: &str,
) -> Result<(), String> {
    let lang = settings.read().unwrap().language;
    upload_workout_connect(app, connect, settings, workout)
        .await
        .map_err(|e| {
            report_error(
                format!("{workout}: {e}"),
                lang,
                "error_workout_send_connect",
                "Error sending workout to Garmin Connect",
            )
        })
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
            database.run_in_transaction(|tx| {
                Ok(WorkoutDeviceRepository::insert()
                    .or_replace()
                    .item(&mut entry)
                    .execute_in(tx)?)
            })
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

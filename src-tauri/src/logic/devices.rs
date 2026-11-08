use std::collections::{HashMap, HashSet};

use chrono::{Datelike, Local, TimeZone, Timelike};
use futures_util::StreamExt;
use mtp_rs::{MtpDevice, mtp::HotplugEvent};
use rusqlite_orm::{
    dao::Repository,
    database::DatabasePool,
    errors::DatabaseError,
    types::{order_by::OrderBy, where_clause::Where},
};

use tauri::{AppHandle, Emitter, Manager, Runtime};
use tauri_plugin_log::log::{error, info};

use crate::{
    ConnectLock, MtpLock, SettingsLock,
    dao::{
        device::{self, Device, DeviceRepository},
        exercise::{self, ExerciseRepository},
        session::{self, SessionRepository},
        settings::WeightUnit,
        workout::{self, Workout, WorkoutRepository},
        workout_device::{self, WorkoutDevice, WorkoutDeviceRepository},
        workout_step::{self, WorkoutStepRepository},
    },
    dto::{
        devices::DeviceListItem,
        notifications::{NotificationDefinition, NotificationKind},
    },
    fit::{FitFile, parser::FitParser, writer::FitWriter},
    logic::{
        connect::_send_to_connect,
        new_pending_dir,
        notifications::show_notification,
        report_error, run_blocking,
        sessions::{import_pending_sessions, update_pending_geolocation},
        store_pending_files,
    },
    utils::{
        constants,
        translations::{Languages, translate, translate_and_replace},
    },
};

/// Spawns a background task that watches USB hotplug events and keeps the device list/DB/frontend in sync.
pub fn start_device_watcher<R: Runtime>(app: &AppHandle<R>) {
    info!("Starting device monitor...");
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        match mtp_rs::mtp::watch_devices() {
            Ok(w) => {
                let mut devices: HashMap<String, Device> = HashMap::new();
                let mut locations: HashMap<u64, String> = HashMap::new();

                let mut watch = w;
                while let Some(event) = watch.next().await {
                    match event {
                        HotplugEvent::Arrived(info) => {
                            if info.vendor_id != constants::MTP_GARMIN_VENDOR_ID {
                                continue;
                            }

                            if let Ok(device) = MtpDevice::open_by_location(info.location_id).await
                            {
                                let parsed_device = Device::try_from(device.device_info());
                                let _ = device.close().await;

                                if let Ok(mut device) = parsed_device {
                                    locations.insert(info.location_id, device.serial.clone());
                                    on_device_connected(&app, &mut device, &mut devices).await
                                }
                            }
                        }
                        HotplugEvent::Left(info) => {
                            if let Some(serial) = locations.remove(&info.location_id) {
                                on_device_disconnected(&app, &serial, &mut devices).await
                            }
                        }
                    }
                }
            }
            Err(e) => {
                error!("Could not initialize device monitor: {e}");
            }
        };
    });
}

async fn on_device_disconnected<R: Runtime>(
    app: &AppHandle<R>,
    serial: &str,
    devices: &mut HashMap<String, Device>,
) {
    if let Some(device) = devices.remove(serial) {
        let payload: DeviceListItem = DeviceListItem::from(&device);
        let _ = app.emit("device_disconnected", payload);

        let lang = {
            let settings_state = app.state::<SettingsLock>();
            let settings = settings_state.read().unwrap();
            settings.language
        };

        info!("Disconnected Garmin {} ({})", device.model, device.serial);
        show_notification(NotificationDefinition {
            title: translate("device_disconnected", lang),
            body: format!("Garmin {}", device.model),
            kind: NotificationKind::Temporal,
        });
    }
}

async fn on_device_connected<R: Runtime>(
    app: &AppHandle<R>,
    device: &mut Device,
    devices: &mut HashMap<String, Device>,
) {
    // The initial scan and the hotplug watcher can both report the same device.
    if devices.contains_key(&device.serial) {
        return;
    }

    let lang = {
        let settings_state = app.state::<SettingsLock>();
        let settings = settings_state.read().unwrap();
        settings.language
    };
    let db = app.state::<DatabasePool>();
    let enrol_res = db.run_in_transaction(|tx| {
        let _: () = if let Some(device) = DeviceRepository::select_by_id_in(tx, &device.serial)? {
            if !device.usb {
                DeviceRepository::update()
                    .set(device::entity::columns::USB, true.into())
                    .where_(Where::Eq(
                        device::entity::columns::SERIAL,
                        device.serial.clone().into(),
                    ))
                    .execute_in(tx)
                    .map(|_| ())?
            }
        } else {
            DeviceRepository::insert()
                .item(device)
                .execute_in(tx)
                .map(|_| ())?
        };
        Ok(())
    });
    if let Err(e) = enrol_res {
        error!(
            "Error enrolling Garmin {} ({}): {}",
            device.model, device.serial, e
        );
        return;
    }

    info!("Connected Garmin {} ({})", device.model, device.serial);
    devices.insert(device.serial.clone(), device.clone());

    show_notification(NotificationDefinition {
        title: translate("device_connected", lang),
        body: translate_and_replace("syncing_device", &["Garmin", &device.model], lang),
        kind: NotificationKind::Temporal,
    });

    let payload: DeviceListItem = DeviceListItem::from(&device.clone());
    let _ = app.emit("device_connected", payload);

    let serial = &device.serial;
    with_loading(app, async {
        let imported = _import_from_device(app, serial).await.unwrap_or(0);

        download_pending_workouts(app, serial).await;
        delete_pending_workouts(app, serial).await;
        push_pending_workouts(app, serial).await;

        imported
    })
    .await;
}

/// Runs a sync between `start_loading` and `finish_loading`, then emits `sessions_added` if it
/// imported any session. Each event is emitted only here, as the TS listener is generated per call.
async fn with_loading<R: Runtime>(app: &AppHandle<R>, sync: impl Future<Output = usize>) {
    let _ = app.emit("start_loading", ());
    let imported = sync.await;
    let _ = app.emit("finish_loading", ());
    if imported > 0 {
        let _ = app.emit("sessions_added", ());
    }
}

/// Uploads the pending workouts of every device plugged in by USB. The rest keep them pending for
/// the next plug: Connect only gets the workout itself, never a delivery to a device.
pub async fn push_pending_to_usb<R: Runtime>(app: &AppHandle<R>) {
    let mtp_client = app.state::<MtpLock>();
    // Bound in its own statement so the MTP lock is released before pushing
    let connected = mtp_client.lock().await.get_connected_devices().await;
    match connected {
        Ok(devices) => {
            for device in devices {
                push_pending_workouts(app, &device.serial).await;
            }
        }
        Err(e) => error!("Error listing connected devices: {e}"),
    }
}

/// Devices linked to the Connect account where `workout` is still pending.
pub fn pending_connect_devices(
    conn: &rusqlite_orm::rusqlite::Connection,
    workout: &str,
) -> rusqlite_orm::errors::Result<Vec<String>> {
    Ok(WorkoutDeviceRepository::select()
        .columns(&[workout_device::entity::columns::DEVICE])
        .where_(Where::And(vec![
            Where::Eq(workout_device::entity::columns::WORKOUT, workout.into()),
            Where::Eq(workout_device::entity::columns::PENDING, true.into()),
            Where::InSub(
                workout_device::entity::columns::DEVICE,
                DeviceRepository::select()
                    .distinct(&[device::entity::columns::SERIAL])
                    .where_(Where::Eq(device::entity::columns::CONNECT, true.into()))
                    .to_subquery(),
            ),
        ]))
        .fetch_in(conn)?
        .iter()
        .map(|r| r.get_text(workout_device::entity::columns::DEVICE.as_ref()))
        .collect())
}

/// Delivers a just enabled `workout` through Connect to the linked devices still waiting for it,
/// so call it after `push_pending_to_usb` for USB to take precedence.
pub async fn send_enabled_workout_to_connect_devices<R: Runtime>(
    app: &AppHandle<R>,
    workout: &str,
) {
    if app
        .state::<ConnectLock>()
        .lock()
        .await
        .logged_user()
        .is_none()
    {
        return;
    }

    // Held for the whole run: a waiting caller re-reads the queue after this one has cleared it
    let _guard = PUSH_LOCK.lock().await;
    let name = workout.to_string();
    let serials = match run_blocking(app, move |db| {
        db.run_in_connection(|conn| Ok(pending_connect_devices(conn, &name)?))
    })
    .await
    {
        Ok(serials) => serials,
        Err(e) => {
            error!("Error fetching Connect devices waiting for workout '{workout}': {e}");
            return;
        }
    };

    let lang = app.state::<SettingsLock>().read().unwrap().language;
    for serial in serials {
        if let Err(e) = _send_to_connect(app, workout, &serial).await {
            error!("Error pushing pending workout '{workout}' to {serial}: {e}");
            continue;
        }
        finish_push(app, workout, &serial, lang).await;
    }
}

/// Delete the pending workouts of every currently connected device.
pub async fn delete_pending_from_connected<R: Runtime>(app: &AppHandle<R>) {
    let mtp_client = app.state::<MtpLock>();
    // Bound in its own statement so the MTP lock is released before pushing
    let connected = mtp_client.lock().await.get_connected_devices().await;
    match connected {
        Ok(devices) => {
            for device in devices {
                delete_pending_workouts(app, &device.serial).await;
            }
        }
        Err(e) => error!("Error listing connected devices: {e}"),
    }
}

/// Serializes pending pushes so concurrent triggers don't upload the same queue twice.
static PUSH_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// Uploads by USB to `device` the workouts queued while it was unplugged, removing each queue entry once sent.
async fn push_pending_workouts<R: Runtime>(app: &AppHandle<R>, serial: &str) {
    // Held for the whole run: a waiting caller re-reads the queue after this one has cleared it
    let _guard = PUSH_LOCK.lock().await;
    let serial = serial.to_string();
    let serial_q = serial.clone();
    let pending = match run_blocking(app, move |db| {
        WorkoutDeviceRepository::select()
            .columns(&[workout_device::entity::columns::WORKOUT])
            .where_(Where::And(vec![
                Where::InSub(
                    workout_device::entity::columns::WORKOUT,
                    WorkoutRepository::select()
                        .distinct(&[workout::entity::columns::NAME])
                        .where_(Where::Eq(workout::entity::columns::ENABLED, true.into()))
                        .to_subquery(),
                ),
                Where::Eq(
                    workout_device::entity::columns::DEVICE,
                    serial_q.clone().into(),
                ),
                Where::Eq(workout_device::entity::columns::PENDING, true.into()),
            ]))
            .order_by(OrderBy::Asc(workout_device::entity::columns::WORKOUT))
            .fetch(db)
            .map(|rows| {
                rows.iter()
                    .map(|r| r.get_text(workout_device::entity::columns::WORKOUT.as_ref()))
                    .collect::<Vec<_>>()
            })
    })
    .await
    {
        Ok(pending) => pending,
        Err(e) => {
            error!("Error fetching pending workouts for pushing to {serial}: {e}");
            return;
        }
    };
    if pending.is_empty() {
        return;
    }

    let (lang, weight_unit) = {
        let state = app.state::<SettingsLock>();
        let settings = state.read().unwrap();
        (settings.language, settings.weight_unit)
    };
    for workout in pending {
        if let Err(e) = send_workout_to_device(app, lang, weight_unit, &workout, &serial).await {
            error!("Error pushing pending workout '{workout}' to {serial}: {e}");
            continue;
        }
        finish_push(app, &workout, &serial, lang).await;
    }
}

/// Removes the queue entry of a workout just sent to the device `serial` and notifies it.
async fn finish_push<R: Runtime>(app: &AppHandle<R>, workout: &str, serial: &str, lang: Languages) {
    let device = serial.to_string();
    let wkt = workout.to_string();
    if let Err(e) = run_blocking(app, move |db| {
        WorkoutDeviceRepository::update()
            .set(workout_device::entity::columns::PENDING, false.into())
            .where_(Where::And(vec![
                Where::Eq(workout_device::entity::columns::WORKOUT, wkt.clone().into()),
                Where::Eq(
                    workout_device::entity::columns::DEVICE,
                    device.clone().into(),
                ),
            ]))
            .execute(db)
    })
    .await
    {
        error!("Error clearing pending upload entry for {serial}: {e}");
    }

    info!("Workout '{workout}' sent to device {serial}");
    show_notification(NotificationDefinition {
        title: workout.to_string(),
        body: translate("ok_workout_send", lang),
        kind: NotificationKind::Temporal,
    });
}

/// Deletes from `device` the disabled workouts queued while it was disconnected, removing each queue entry once deleted.
async fn delete_pending_workouts<R: Runtime>(app: &AppHandle<R>, serial: &str) {
    // Held for the whole run: a waiting caller re-reads the queue after this one has cleared it
    let _guard = PUSH_LOCK.lock().await;
    let serial = serial.to_string();
    let serial_q = serial.clone();
    let pending = match run_blocking(app, move |db| {
        WorkoutDeviceRepository::select()
            .columns(&[workout_device::entity::columns::WORKOUT])
            .where_(Where::And(vec![
                Where::InSub(
                    workout_device::entity::columns::WORKOUT,
                    WorkoutRepository::select()
                        .distinct(&[workout::entity::columns::NAME])
                        .where_(Where::Eq(workout::entity::columns::ENABLED, false.into()))
                        .to_subquery(),
                ),
                Where::Eq(
                    workout_device::entity::columns::DEVICE,
                    serial_q.clone().into(),
                ),
                Where::Eq(workout_device::entity::columns::PENDING, true.into()),
            ]))
            .order_by(OrderBy::Asc(workout_device::entity::columns::WORKOUT))
            .fetch(db)
            .map(|rows| {
                rows.iter()
                    .map(|r| r.get_text(workout_device::entity::columns::WORKOUT.as_ref()))
                    .collect::<Vec<_>>()
            })
    })
    .await
    {
        Ok(pending) => pending,
        Err(e) => {
            error!("Error fetching pending workouts for deleting from {serial}: {e}");
            return;
        }
    };
    if pending.is_empty() {
        return;
    }

    let lang = {
        let state = app.state::<SettingsLock>();
        let settings = state.read().unwrap();
        settings.language
    };
    for workout in pending {
        match delete_workout_from_device(app, &workout, &serial).await {
            Ok(deleted) => {
                let device = serial.clone();
                let wkt = workout.clone();
                if let Err(e) = run_blocking(app, move |db| {
                    WorkoutDeviceRepository::update()
                        .set(workout_device::entity::columns::PENDING, false.into())
                        .where_(Where::And(vec![
                            Where::Eq(workout_device::entity::columns::WORKOUT, wkt.clone().into()),
                            Where::Eq(
                                workout_device::entity::columns::DEVICE,
                                device.clone().into(),
                            ),
                        ]))
                        .execute(db)
                })
                .await
                {
                    error!("Error clearing pending deletion entry for {serial}: {e}");
                }
                if deleted {
                    info!("Workout '{workout}' deleted from device {serial}");
                    show_notification(NotificationDefinition {
                        title: workout.to_string(),
                        body: translate("ok_workout_delete", lang),
                        kind: NotificationKind::Temporal,
                    });
                }
            }
            Err(e) => {
                error!("Error deleting workout '{workout}' from {serial}: {e}");
                continue;
            }
        }
    }
}

/// Lists the workouts recorded in sessions of `serial` that have no steps stored yet.
async fn list_pending_workouts<R: Runtime>(
    app: &AppHandle<R>,
    serial: &str,
) -> Result<Vec<String>, DatabaseError> {
    let serial_q = serial.to_string();
    let rows = run_blocking(app, move |db| {
        WorkoutRepository::select()
            .columns(&[workout::entity::columns::NAME])
            .where_(Where::And(vec![
                Where::NotInSub(
                    workout::entity::columns::NAME,
                    WorkoutStepRepository::select()
                        .distinct(&[workout_step::entity::columns::WORKOUT])
                        .to_subquery(),
                ),
                Where::InSub(
                    workout::entity::columns::NAME,
                    SessionRepository::select()
                        .distinct(&[session::entity::columns::WORKOUT])
                        .where_(Where::Eq(
                            session::entity::columns::DEVICE,
                            serial_q.clone().into(),
                        ))
                        .to_subquery(),
                ),
            ]))
            .fetch(db)
    })
    .await?;

    Ok(rows
        .iter()
        .map(|r| r.get_text(workout::entity::columns::NAME.as_ref()))
        .collect())
}

/// Stores the steps of the `pending_workouts` found among the `.fit` files fetched from the
/// device. Those that fail to parse or to be stored are kept as pending files.
fn store_pending_workouts<R: Runtime>(
    app: &AppHandle<R>,
    db: &DatabasePool,
    files: &[FitFile],
    serial: &str,
    pending_workouts: &[String],
) {
    // Every workout on the device gets downloaded; those not pending are just discarded
    let mut failed = Vec::new();
    let mut workouts_to_insert = Vec::new();
    for file in files {
        match Workout::try_from(FitParser::from_fit_file(file.clone())) {
            Ok(workout)
                if !workout.steps.is_empty() && pending_workouts.contains(&workout.name) =>
            {
                workouts_to_insert.push((workout, file))
            }
            Ok(_) => {}
            Err(e) => {
                error!("Error parsing workout file {}: {e}", file.name);
                failed.push(file);
            }
        }
    }

    let mut workout_names: Vec<String> = Vec::new();
    for (mut workout, file) in workouts_to_insert {
        let res = db.run_in_transaction(|tx| {
            WorkoutRepository::update()
                .set(
                    workout::entity::columns::NOTES,
                    workout.notes.clone().into(),
                )
                .where_(Where::Eq(
                    workout::entity::columns::NAME,
                    workout.name.clone().into(),
                ))
                .execute_in(tx)?;

            let mut insert = WorkoutStepRepository::insert();
            for step in &mut workout.steps {
                insert = insert.item(step);
            }

            insert.execute_in(tx)?;

            Ok(())
        });

        if let Err(e) = res {
            error!(
                "Error storing workout {} from {}: {}",
                workout.name, serial, e
            );
            failed.push(file);
        } else {
            workout_names.push(workout.name.clone());
        }
    }

    store_pending_files(&constants::PENDING_DOWNLOAD_WORKOUTS_FOLDER, serial, failed);

    if !workout_names.is_empty() {
        let _ = app.emit("added_workout_steps", workout_names);
    }
}

/// Downloads from `device` the step definitions of workouts that have sessions recorded on it but no steps stored yet.
async fn download_pending_workouts<R: Runtime>(app: &AppHandle<R>, serial: &str) {
    let pending_workouts = match list_pending_workouts(app, serial).await {
        Ok(pending) => pending,
        Err(e) => {
            error!("Error listing pending workouts for {}: {e}", serial);
            return;
        }
    };

    if pending_workouts.is_empty() {
        return;
    }

    let mtp_client = app.state::<MtpLock>();

    // Locked only for the download, so a concurrent sync is not held back by DB work.
    let downloaded = mtp_client.lock().await.download_workouts(serial).await;

    match downloaded {
        Ok(files) => {
            let app_cl = app.clone();
            let serial = serial.to_string();
            run_blocking(app, move |db| {
                store_pending_workouts(&app_cl, db, &files, &serial, &pending_workouts)
            })
            .await;
        }
        Err(e) => error!("Error downloading workouts from {}: {e}", serial),
    }
}

pub async fn send_to_device<R: Runtime>(
    app: &AppHandle<R>,
    settings: &SettingsLock,
    workout: &str,
    serial: &str,
) -> Result<(), String> {
    let (lang, weight_unit) = {
        let settings = settings.read().unwrap();
        (settings.language, settings.weight_unit)
    };

    let mtp_client = app.state::<MtpLock>();
    let connected = mtp_client
        .lock()
        .await
        .is_device_connected(serial)
        .await
        .map_err(|e| e.to_string())?;

    let res: Result<bool, String> = if connected {
        match send_workout_to_device(app, lang, weight_unit, workout, serial).await {
            Ok(_) => {
                // Register the pair so later changes (e.g. progressions) get re-sent to this device
                let mut entry = WorkoutDevice {
                    device: serial.to_string(),
                    workout: workout.to_string(),
                    pending: false,
                };

                if let Err(e) = run_blocking(app, move |database| {
                    WorkoutDeviceRepository::insert()
                        .or_ignore()
                        .item(&mut entry)
                        .execute(database)
                })
                .await
                {
                    error!("Error registering workout '{workout}' for device {serial}: {e}");
                }

                Ok(true)
            }
            Err(e) => Err(e),
        }
    } else {
        let mut entry = WorkoutDevice {
            device: serial.to_string(),
            workout: workout.to_string(),
            pending: true,
        };

        run_blocking(app, move |database| {
            WorkoutDeviceRepository::insert()
                .or_replace()
                .item(&mut entry)
                .execute(database)
        })
        .await
        .map_err(|e| e.to_string())?;

        Ok(false)
    };

    match res {
        Ok(true) => {
            info!("Workout '{}' sent to device {}", workout, serial);
            show_notification(NotificationDefinition {
                title: workout.to_string(),
                body: translate("ok_workout_send", lang),
                kind: NotificationKind::Temporal,
            });
            Ok(())
        }
        Ok(false) => {
            info!(
                "Workout '{}' pending to be sent to device {}",
                workout, serial
            );
            show_notification(NotificationDefinition {
                title: workout.to_string(),
                body: translate("pending_workout_send", lang),
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

async fn send_workout_to_device<R: Runtime>(
    app: &AppHandle<R>,
    lang: Languages,
    weight_unit: WeightUnit,
    workout: &str,
    serial: &str,
) -> Result<(), String> {
    let name = workout.to_string();
    let workout = run_blocking(app, move |database| {
        database.run_in_connection(|conn| {
            let mut wk = WorkoutRepository::select_by_id_in(conn, &name)?.unwrap();
            wk.fetch_steps_relationship_in(conn)?;

            // Exercise steps without their own notes inherit the exercise notes
            let pending: Vec<(usize, (u16, u16))> = wk
                .steps
                .iter()
                .enumerate()
                .filter_map(|(idx, step)| {
                    if step.notes.as_deref().is_none_or(str::is_empty)
                        && let Some(id) = step.ex_id
                        && let Some(cat) = step.ex_cat
                    {
                        Some((idx, (cat, id)))
                    } else {
                        None
                    }
                })
                .collect();

            if !pending.is_empty() {
                let pks: HashSet<(u16, u16)> = pending.iter().map(|(_, pk)| *pk).collect();
                let notes = ExerciseRepository::select()
                    .where_(Where::InMultiple(
                        vec![
                            exercise::entity::columns::CATEGORY,
                            exercise::entity::columns::ID,
                        ],
                        pks.iter()
                            .map(|(cat, id)| vec![(*cat).into(), (*id).into()])
                            .collect(),
                    ))
                    .fetch_in(conn)?
                    .into_iter()
                    .filter_map(|e| Some(((e.category, e.id), e.notes?)))
                    .collect::<HashMap<_, _>>();

                for (idx, pk) in pending {
                    if let Some(note) = notes.get(&pk) {
                        wk.steps[idx].notes = Some(note.clone());
                    }
                }
            }

            Ok(wk)
        })
    })
    .await
    .map_err(|e| e.to_string())?;

    let dir = new_pending_dir(&constants::PENDING_UPLOAD_WORKOUTS_FOLDER, serial);
    tokio::fs::create_dir_all(&dir)
        .await
        .map_err(|e| e.to_string())?;
    // File name is kept as-is since it becomes the workout file name on the device
    let path = dir.join(workout.get_workout_file_name());
    if let Err(e) = FitWriter::from(&workout).write(path.clone(), lang, weight_unit) {
        let _ = tokio::fs::remove_dir_all(&dir).await;
        return Err(e.to_string());
    }

    let mtp_client = app.state::<MtpLock>();
    let upload = mtp_client
        .lock()
        .await
        .upload_workout(serial, path.clone())
        .await
        .map_err(|e| e.to_string());

    let _ = tokio::fs::remove_dir_all(dir).await;

    upload
}

async fn delete_workout_from_device<R: Runtime>(
    app: &AppHandle<R>,
    workout: &str,
    serial: &str,
) -> Result<bool, String> {
    let mtp_client = app.state::<MtpLock>();
    mtp_client
        .lock()
        .await
        .delete_workout(serial, &Workout::file_name_for(workout))
        .await
        .map_err(|e| e.to_string())
}

/// Downloads new activity files from the given device since its last sync and imports them.
pub async fn _import_from_device<R: Runtime>(
    app: &AppHandle<R>,
    serial: &str,
) -> Result<usize, String> {
    info!("Starting import from device with S/N {}", serial);
    let mut latest_date = "1970-01-01-00-00-00".to_string();
    let lang = app.state::<SettingsLock>().read().unwrap().language;
    let mut device = {
        let app = app.clone();
        let serial = serial.to_string();
        tokio::task::spawn_blocking(move || {
            let db = app.state::<DatabasePool>();
            DeviceRepository::select_by_id(&db, &serial).map(Option::unwrap)
        })
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())
    }?;

    if let Some(latest) = device.last_activity_sync {
        let latest = Local.timestamp_opt(latest as i64, 0).unwrap();
        latest_date = format!(
            "{:04}-{:02}-{:02}-{:02}-{:02}-{:02}",
            latest.year(),
            latest.month(),
            latest.day(),
            latest.hour(),
            latest.minute(),
            latest.second(),
        );
    }

    info!(
        "Fetching from device activity files after {}...",
        latest_date
    );
    let mut res: Result<Vec<u32>, DatabaseError> = Ok(Vec::new());
    let mtp_client = app.state::<MtpLock>();
    let mtp_client = mtp_client.lock().await;

    if let Ok(files) = mtp_client
        .download_activities_since(serial, latest_date)
        .await
    {
        drop(mtp_client);
        let app_cpy = app.clone();
        res = tokio::task::spawn_blocking(move || {
            let db = app_cpy.state::<DatabasePool>();
            let now = Local::now().timestamp() as u32;
            import_pending_sessions(&app_cpy, &db, &files, &mut device, lang, Some(now))
        })
        .await
        .expect("blocking DB task panicked");
    }

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

pub async fn get_registered_devices<R: Runtime>(
    app: &AppHandle<R>,
) -> Result<Vec<DeviceListItem>, String> {
    Ok(
        run_blocking(app, |database| DeviceRepository::select().fetch(database))
            .await
            .map_err(|e| e.to_string())?
            .iter()
            .filter_map(|d| {
                if d.serial != constants::CONNECT_DEVICE_SERIAL {
                    Some(DeviceListItem::from(d))
                } else {
                    None
                }
            })
            .collect(),
    )
}

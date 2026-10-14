use std::{
    fs,
    time::{SystemTime, UNIX_EPOCH},
};

use chrono::{Datelike, Local, TimeZone, Timelike};
use nusb::hotplug::HotplugEvent;
use rusqlite_orm::{
    dao::Repository,
    database::DatabasePool,
    errors::DatabaseError,
    types::{value::Value, where_clause::Where},
};
use strength_tracker_rs_macros::traced_command;
use tokio_stream::StreamExt;

use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_log::log::{error, info};

use crate::{
    SettingsLock,
    dao::{
        device::{Device, DeviceRepository},
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
    fit::{parser::FitParser, writer::FitWriter},
    logic::{
        notifications::show_notification,
        report_error, run_blocking,
        sessions::{import_file_list, update_pending_geolocation},
    },
    mtp::MTP_CLIENT_INST,
    utils::{
        constants,
        translations::{Languages, translate, translate_and_replace},
    },
};

/// Spawns a background task that watches USB hotplug events and keeps the device list/DB/frontend in sync.
pub fn start_device_watcher(app: AppHandle) {
    info!("Starting device monitor...");
    tauri::async_runtime::spawn(async move {
        let mut devices: Vec<DeviceListItem> = Vec::new();

        match nusb::watch_devices() {
            Ok(w) => {
                mtp_dev_check_and_sync(app.clone(), &mut devices).await;
                let mut watch = w;
                while let Some(event) = watch.next().await {
                    match event {
                        HotplugEvent::Connected(_) | HotplugEvent::Disconnected(_) => {
                            mtp_dev_check_and_sync(app.clone(), &mut devices).await;
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

/// Diffs the currently connected Garmin devices against `devices`, enrolling new ones in the DB, emitting connect/disconnect events, and triggering auto-sync for newly connected devices.
async fn mtp_dev_check_and_sync(app: AppHandle, devices: &mut Vec<DeviceListItem>) {
    let mut devs_to_sync = Vec::new();
    let mtp_client = MTP_CLIENT_INST.lock().await;
    let connected = mtp_client
        .get_connected_devices()
        .await
        .map_err(|e| e.to_string());
    drop(mtp_client);

    if let Ok(cur_dev) = connected {
        let already_known: Vec<String> = devices.iter().map(|d| d.serial_number.clone()).collect();

        let db = app.state::<DatabasePool>();

        let newly_enrolled = db
            .run_in_transaction(|tx| {
                let mut enrolled = Vec::new();
                for device in &cur_dev {
                    if !already_known.contains(&device.serial_number) {
                        let enrol_err =
                            match DeviceRepository::select_by_id_in(tx, &device.serial_number) {
                                Ok(None) => DeviceRepository::insert()
                                    .item(&mut Device::from(device))
                                    .execute_in(tx)
                                    .err(),
                                Ok(Some(_)) => None,
                                Err(e) => Some(e),
                            };

                        match enrol_err {
                            Some(e) => {
                                error!(
                                    "Error enrolling {} {} ({}): {}",
                                    device.manufacturer, device.model, device.serial_number, e
                                );
                            }
                            None => enrolled.push(device.clone()),
                        }
                    }
                }

                Ok(enrolled)
            })
            .unwrap_or_default();

        let lang = {
            let settings_state = app.state::<SettingsLock>();
            let settings = settings_state.read().unwrap();
            settings.language
        };
        for device in &newly_enrolled {
            info!(
                "Connected {} {} ({})",
                device.manufacturer, device.model, device.serial_number
            );
            devices.push(device.clone());

            devs_to_sync.push(device.serial_number.clone());
            show_notification(NotificationDefinition {
                title: translate("device_connected", lang),
                body: translate_and_replace(
                    "syncing_device",
                    &[&device.manufacturer, &device.model],
                    lang,
                ),
                kind: NotificationKind::Temporal,
            });

            let payload: DeviceListItem = device.clone();
            let _ = app.emit("device_connected", payload);
        }

        for device in devices.iter() {
            if !cur_dev
                .iter()
                .any(|d| d.serial_number == device.serial_number)
            {
                let payload: DeviceListItem = device.clone();
                let _ = app.emit("device_disconnected", payload);

                info!(
                    "Disconnected {} {} ({})",
                    device.manufacturer, device.model, device.serial_number
                );
                show_notification(NotificationDefinition {
                    title: translate("device_disconnected", lang),
                    body: format!("{} {}", device.manufacturer, device.model),
                    kind: NotificationKind::Temporal,
                });
            }
        }

        devices.retain(|d| cur_dev.iter().any(|cd| cd.serial_number == d.serial_number));
    }

    if !devs_to_sync.is_empty() {
        let _ = app.emit("start_loading", ());
        let mut imported = 0;
        for dev in devs_to_sync {
            if let Ok(i) = _import_from_device(&app, &dev).await {
                imported += i;
            }

            download_pending_workouts(&app, &dev).await;
            push_pending_workouts(&app, &dev).await;
        }
        let _ = app.emit("finish_loading", ());
        if imported > 0 {
            let _ = app.emit("sessions_added", ());
        }
    }
}

/// Uploads to `device` the workouts queued while it was disconnected, removing each queue entry once sent.
async fn push_pending_workouts(app: &AppHandle, serial: &str) {
    let serial = serial.to_string();
    let serial_q = serial.clone();
    let pending = match run_blocking(app.clone(), move |db| {
        db.run_in_connection(|conn| {
            Ok(WorkoutDeviceRepository::select()
                .columns(&[workout_device::entity::columns::WORKOUT])
                .where_(Where::Eq(
                    workout_device::entity::columns::DEVICE,
                    serial_q.clone().into(),
                ))
                .fetch_in(conn)?
                .iter()
                .filter_map(
                    |r| match r.get(workout_device::entity::columns::WORKOUT.as_ref()) {
                        Some(Value::Text(name)) => Some(name.clone()),
                        _ => None,
                    },
                )
                .collect::<Vec<_>>())
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
        if let Err(e) =
            send_workout_to_device(app.clone(), lang, weight_unit, &workout, &serial).await
        {
            error!("Error pushing pending workout '{workout}' to {serial}: {e}");
            continue;
        }
        info!("Pending workout '{workout}' sent to device {serial}");
        show_notification(NotificationDefinition {
            title: workout.to_string(),
            body: translate("ok_workout_send", lang),
            kind: NotificationKind::Temporal,
        });

        let device = serial.clone();
        if let Err(e) = run_blocking(app.clone(), move |db| {
            db.run_in_transaction(|tx| {
                Ok(WorkoutDeviceRepository::delete()
                    .where_(Where::And(vec![
                        Where::Eq(
                            workout_device::entity::columns::WORKOUT,
                            workout.clone().into(),
                        ),
                        Where::Eq(
                            workout_device::entity::columns::DEVICE,
                            device.clone().into(),
                        ),
                    ]))
                    .execute_in(tx)?)
            })
        })
        .await
        {
            error!("Error clearing pending upload entry for {serial}: {e}");
        }
    }
}

/// Downloads from `device` the step definitions of workouts that have sessions recorded on it but no steps stored yet.
async fn download_pending_workouts(app: &AppHandle, serial: &str) {
    let serial = serial.to_string();
    let serial_q = serial.clone();
    let pending_workouts = match run_blocking(app.clone(), move |db| {
        db.run_in_connection(|conn| {
            let rows = WorkoutRepository::select()
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
                .fetch_in(conn)?;
            Ok(rows)
        })
    })
    .await
    {
        Ok(rows) => rows
            .iter()
            .filter_map(|r| match r.get(workout::entity::columns::NAME.as_ref()) {
                Some(Value::Text(name)) => Some(name.clone()),
                _ => None,
            })
            .collect::<Vec<_>>(),
        Err(e) => {
            error!("Error listing pending workouts for {}: {e}", serial);
            return;
        }
    };

    if pending_workouts.is_empty() {
        return;
    }

    let dst_dir = std::env::temp_dir().join(format!(
        "{}-{}",
        constants::MTP_TMP_DIR_PREFIX,
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis()
    ));

    // Locked only for the download, so a concurrent sync is not held back by DB work.
    let downloaded = MTP_CLIENT_INST
        .lock()
        .await
        .download_workouts(&serial, dst_dir.clone())
        .await;

    match downloaded {
        Ok(()) => {
            let src_dir = dst_dir.clone();
            let app_cl: AppHandle = app.clone();
            run_blocking(app.clone(), move |db| {
                let mut workouts_to_insert = Vec::new();
                if let Ok(read_dir) = fs::read_dir(&src_dir) {
                    for entry in read_dir.flatten() {
                        if let Ok(parser) = FitParser::from_file(entry.path())
                            && let Ok(workout) = Workout::try_from(parser)
                            && !workout.steps.is_empty()
                            && pending_workouts.contains(&workout.name)
                        {
                            workouts_to_insert.push(workout);
                        }
                    }
                }

                let mut workout_names: Vec<String> = Vec::new();
                for workout in &mut workouts_to_insert {
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
                    } else {
                        workout_names.push(workout.name.clone());
                    }
                }

                if !workout_names.is_empty() {
                    let _ = app_cl.emit("added_workout_steps", workout_names);
                }
            })
            .await;
        }
        Err(e) => error!("Error downloading workouts from {}: {e}", serial),
    }

    let _ = fs::remove_dir_all(&dst_dir);
}

#[traced_command]
#[tauri::command]
pub async fn send_to_device(
    app: AppHandle,
    settings: State<'_, SettingsLock>,
    workout: &str,
    serial: &str,
) -> Result<(), String> {
    let (lang, weight_unit) = {
        let settings = settings.read().unwrap();
        (settings.language, settings.weight_unit)
    };

    let connected = MTP_CLIENT_INST
        .lock()
        .await
        .is_device_connected(serial)
        .await
        .map_err(|e| e.to_string())?;

    let res: Result<bool, String> = if connected {
        send_workout_to_device(app, lang, weight_unit, workout, serial)
            .await
            .map(|_| true)
    } else {
        let mut entry = WorkoutDevice {
            device: serial.to_string(),
            workout: workout.to_string(),
        };

        run_blocking(app, move |database| {
            database.run_in_transaction(|tx| {
                Ok(WorkoutDeviceRepository::insert()
                    .or_ignore()
                    .item(&mut entry)
                    .execute_in(tx)?)
            })
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

async fn send_workout_to_device(
    app: AppHandle,
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

            Ok(wk)
        })
    })
    .await
    .map_err(|e| e.to_string())?;

    let path = std::env::temp_dir().join(workout.get_workout_file_name());
    FitWriter::from(&workout)
        .write(path.clone(), lang, weight_unit)
        .map_err(|e| e.to_string())?;

    let upload = MTP_CLIENT_INST
        .lock()
        .await
        .upload_workout(serial, path.clone())
        .await
        .map_err(|e| e.to_string());

    let _ = tokio::fs::remove_file(path).await;

    upload
}

/// Tauri command wrapper around `_import_from_device`; returns the number of sessions imported.
#[traced_command]
#[tauri::command]
pub async fn import_from_device(app: AppHandle, serial: &str) -> Result<usize, String> {
    _import_from_device(&app, serial).await
}

/// Downloads new activity files from the given device since its last sync and imports them.
pub async fn _import_from_device(app: &AppHandle, serial: &str) -> Result<usize, String> {
    info!("Starting import from device with S/N {}", serial);
    let mut latest_date = "2026-06-08-00-00-00".to_string();
    let lang = app.state::<SettingsLock>().read().unwrap().language;
    let mut device = {
        let app = app.clone();
        let serial = serial.to_string();
        tokio::task::spawn_blocking(move || {
            let db = app.state::<DatabasePool>();
            db.run_in_connection(|conn| {
                let device = DeviceRepository::select_by_id_in(conn, &serial)?;

                Ok(device.unwrap())
            })
        })
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())
    }?;

    if let Some(latest) = device.last_sync {
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
    let mut activities = Vec::new();

    let mtp_client = MTP_CLIENT_INST.lock().await;

    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
    let src_dir = std::env::temp_dir().join(format!(
        "{}-{}",
        constants::MTP_TMP_DIR_PREFIX,
        now.as_millis()
    ));

    let activities_folder = src_dir.join("Activities");

    if mtp_client
        .download_activities_since(serial, latest_date, activities_folder.clone())
        .await
        .is_ok()
    {
        activities = tokio::task::spawn_blocking(move || {
            let mut files = Vec::new();
            if let Ok(read_dir) = fs::read_dir(&activities_folder) {
                for entry in read_dir.flatten() {
                    files.push(entry.path());
                }
            };
            files
        })
        .await
        .map_err(|e| e.to_string())?;

        drop(mtp_client);
        let activities_cpy = activities.clone();
        let app_cpy = app.clone();
        res = tokio::task::spawn_blocking(move || {
            let db = app_cpy.state::<DatabasePool>();
            db.run_in_transaction(move |tx| {
                let res = if !activities_cpy.is_empty() {
                    info!("Fetched {} activity files", activities_cpy.len());
                    import_file_list(tx, &activities_cpy, Some(device.clone()), lang, true)
                } else {
                    Ok(Vec::new())
                }?;

                device.last_sync = Some(Local::now().timestamp() as u32);
                device.update_by_id_in(tx)?;

                Ok(res)
            })
        })
        .await
        .expect("blocking DB task panicked");
    }

    match res {
        Ok(res) => {
            if res.len() == activities.len() {
                let _ = fs::remove_dir_all(src_dir);
            }

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
pub async fn get_registered_devices(app: AppHandle) -> Result<Vec<DeviceListItem>, String> {
    Ok(run_blocking(app, |database| {
        database.run_in_connection(|conn| Ok(DeviceRepository::select().fetch_in(conn)?))
    })
    .await
    .map_err(|e| e.to_string())?
    .into_iter()
    .map(|d| DeviceListItem {
        serial_number: d.serial,
        manufacturer: "Garmin".to_string(),
        model: d.model,
    })
    .collect())
}

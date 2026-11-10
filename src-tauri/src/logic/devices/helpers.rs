use std::collections::HashMap;

use futures_util::StreamExt;
use mtp_rs::{MtpDevice, mtp::HotplugEvent};
use rusqlite_orm::{database::DatabasePool, errors::DatabaseError};
use tauri::{AppHandle, Emitter, Manager, Runtime};
use tauri_plugin_log::log::{error, info};

use super::import_from_device;
use super::queries::{
    clear_pending, enrol_usb_device, pending_connect_devices, pending_queue, store_workout_steps,
    workout_for_device, workouts_without_steps,
};
use crate::{
    ConnectLock, MtpLock, SettingsLock,
    dao::{device::Device, settings::WeightUnit, workout::Workout},
    dto::{
        devices::DeviceListItem,
        notifications::{NotificationDefinition, NotificationKind},
    },
    fit::{FitFile, parser::FitParser, writer::FitWriter},
    logic::{
        connect::helpers::queue_workout_on_connect, new_pending_dir,
        notifications::show_notification, run_blocking, run_blocking_conn, run_blocking_tx,
        store_pending_files,
    },
    utils::{
        constants,
        translations::{Languages, translate, translate_and_replace},
    },
};

/// Spawns a background task that watches USB hotplug events and keeps the device list/DB/frontend in sync.
pub(crate) fn start_device_watcher<R: Runtime>(app: &AppHandle<R>) {
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

pub(super) async fn on_device_disconnected<R: Runtime>(
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

pub(super) async fn on_device_connected<R: Runtime>(
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
    let mut enrolled = device.clone();
    let enrol_res = run_blocking_tx(app, move |_, tx| {
        enrol_usb_device(tx, &mut enrolled)?;
        Ok(enrolled)
    })
    .await;
    match enrol_res {
        Ok(enrolled) => *device = enrolled,
        Err(e) => {
            error!(
                "Error enrolling Garmin {} ({}): {}",
                device.model, device.serial, e
            );
            return;
        }
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
        let imported = import_from_device(app, serial).await.unwrap_or(0);

        download_pending_workouts(app, serial).await;
        delete_pending_workouts(app, serial).await;
        push_pending_workouts(app, serial).await;

        imported
    })
    .await;
}

/// Runs a sync between `start_loading` and `finish_loading`, then emits `sessions_added` if it
/// imported any session. Each event is emitted only here, as the TS listener is generated per call.
pub(super) async fn with_loading<R: Runtime>(
    app: &AppHandle<R>,
    sync: impl Future<Output = usize>,
) {
    let _ = app.emit("start_loading", ());
    let imported = sync.await;
    let _ = app.emit("finish_loading", ());
    if imported > 0 {
        let _ = app.emit("sessions_added", ());
    }
}

/// Uploads the pending workouts of every device plugged in by USB. The rest keep them pending for
/// the next plug: Connect only gets the workout itself, never a delivery to a device.
pub(crate) async fn push_pending_to_usb<R: Runtime>(app: &AppHandle<R>) {
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

/// Delivers a just enabled `workout` through Connect to the linked devices still waiting for it,
/// so call it after `push_pending_to_usb` for USB to take precedence.
pub(crate) async fn send_enabled_workout_to_connect_devices<R: Runtime>(
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
    let serials =
        match run_blocking_conn(app, move |_, conn| pending_connect_devices(conn, &name)).await {
            Ok(serials) => serials,
            Err(e) => {
                error!("Error fetching Connect devices waiting for workout '{workout}': {e}");
                return;
            }
        };

    let lang = app.state::<SettingsLock>().read().unwrap().language;
    for serial in serials {
        if let Err(e) = queue_workout_on_connect(app, workout, &serial).await {
            error!("Error pushing pending workout '{workout}' to {serial}: {e}");
            continue;
        }
        finish_push(app, workout, &serial, lang).await;
    }
}

/// Delete the pending workouts of every currently connected device.
pub(crate) async fn delete_pending_from_connected<R: Runtime>(app: &AppHandle<R>) {
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
pub(super) static PUSH_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// Uploads by USB to `device` the workouts queued while it was unplugged, removing each queue entry once sent.
pub(super) async fn push_pending_workouts<R: Runtime>(app: &AppHandle<R>, serial: &str) {
    // Held for the whole run: a waiting caller re-reads the queue after this one has cleared it
    let _guard = PUSH_LOCK.lock().await;
    let serial = serial.to_string();
    let serial_q = serial.clone();
    let pending = match run_blocking(app, move |db| pending_queue(db, &serial_q, true)).await {
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
pub(super) async fn finish_push<R: Runtime>(
    app: &AppHandle<R>,
    workout: &str,
    serial: &str,
    lang: Languages,
) {
    let device = serial.to_string();
    let wkt = workout.to_string();
    if let Err(e) = run_blocking(app, move |db| clear_pending(db, &wkt, &device)).await {
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
pub(super) async fn delete_pending_workouts<R: Runtime>(app: &AppHandle<R>, serial: &str) {
    // Held for the whole run: a waiting caller re-reads the queue after this one has cleared it
    let _guard = PUSH_LOCK.lock().await;
    let serial = serial.to_string();
    let serial_q = serial.clone();
    let pending = match run_blocking(app, move |db| pending_queue(db, &serial_q, false)).await {
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
                if let Err(e) = run_blocking(app, move |db| clear_pending(db, &wkt, &device)).await
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
pub(super) async fn list_pending_workouts<R: Runtime>(
    app: &AppHandle<R>,
    serial: &str,
) -> Result<Vec<String>, DatabaseError> {
    let serial_q = serial.to_string();
    run_blocking(app, move |db| workouts_without_steps(db, &serial_q)).await
}

/// Stores the steps of the `pending_workouts` found among the `.fit` files fetched from the
/// device. Those that fail to parse or to be stored are kept as pending files.
pub(super) fn store_pending_workouts<R: Runtime>(
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
        let res = db.run_in_transaction(|tx| Ok(store_workout_steps(tx, &mut workout)?));

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
pub(super) async fn download_pending_workouts<R: Runtime>(app: &AppHandle<R>, serial: &str) {
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

pub(super) async fn send_workout_to_device<R: Runtime>(
    app: &AppHandle<R>,
    lang: Languages,
    weight_unit: WeightUnit,
    workout: &str,
    serial: &str,
) -> Result<(), String> {
    let name = workout.to_string();
    let workout = run_blocking_conn(app, move |_, conn| workout_for_device(conn, &name))
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

pub(super) async fn delete_workout_from_device<R: Runtime>(
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

use std::collections::HashMap;

use futures_util::StreamExt;
use mtp_rs::{MtpDevice, mtp::HotplugEvent};
use rusqlite_orm::errors::DatabaseError;
use tauri::{AppHandle, Emitter, Manager, Runtime};
use tauri_plugin_log::log::{error, info};

use super::import_from_device;
use crate::{
    ConnectLock, MtpLock,
    dao::{
        device::Device, settings::WeightUnit, workout::Workout, workout_device::WorkoutDevice,
        workout_planning::WorkoutPlanning,
    },
    dto::{
        devices::DeviceListItem,
        notifications::{NotificationDefinition, NotificationKind},
    },
    fit::{
        FitFile,
        parser::FitParser,
        writer::{FitWriter, ToFitMessages},
    },
    logic::{
        connect::helpers::queue_workout_on_connect, fit_settings, language,
        notifications::show_notification, pool, store_pending_files,
    },
    utils::{
        constants,
        time::{local_midnight, today_local_midnight},
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

/// Records that the device `serial` has the workout, so later changes (e.g. progressions) get
/// re-sent to it. An existing entry is kept as it is.
pub(crate) async fn register_workout_on_device<R: Runtime>(
    app: &AppHandle<R>,
    workout: u16,
    serial: &str,
) -> Result<usize, DatabaseError> {
    let serial = serial.to_string();
    pool(app)
        .run_in_connection(async move |conn| {
            Ok(WorkoutDevice::insert_or_ignore(conn, workout, &serial).await?)
        })
        .await
}

/// Records the workout for the device `serial`, `pending` of being sent or already sent,
/// replacing any existing entry.
pub(crate) async fn set_workout_on_device<R: Runtime>(
    app: &AppHandle<R>,
    workout: u16,
    serial: &str,
    pending: bool,
) -> Result<usize, DatabaseError> {
    let serial = serial.to_string();
    pool(app)
        .run_in_connection(async move |conn| {
            Ok(WorkoutDevice::insert_or_replace(conn, workout, &serial, pending).await?)
        })
        .await
}

/// Name of the workout `id`, for logs and notifications; its id if it can't be read.
pub(crate) async fn workout_name<R: Runtime>(app: &AppHandle<R>, id: u16) -> String {
    pool(app)
        .run_in_connection(async move |conn| Ok(Workout::get(conn, id).await?))
        .await
        .ok()
        .flatten()
        .map_or_else(|| id.to_string(), |workout| workout.name)
}

/// Takes the workout off the queue of the device `serial`, logging a failure.
async fn clear_workout_pending<R: Runtime>(
    app: &AppHandle<R>,
    workout: &str,
    id: u16,
    serial: &str,
) {
    let device = serial.to_string();
    if let Err(e) = pool(app)
        .run_in_connection(async move |conn| {
            Ok(WorkoutDevice::clear_pending(conn, id, &device).await?)
        })
        .await
    {
        error!("Error clearing the queue entry of workout '{workout}' for {serial}: {e}");
    }
}

pub(super) async fn on_device_disconnected<R: Runtime>(
    app: &AppHandle<R>,
    serial: &str,
    devices: &mut HashMap<String, Device>,
) {
    if let Some(device) = devices.remove(serial) {
        let payload: DeviceListItem = DeviceListItem::from(&device);
        let _ = app.emit("device_disconnected", payload);

        let lang = language(app);

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

    let lang = language(app);
    let mut enrolled = device.clone();
    let enrol_res = pool(app)
        .run_in_transaction(async move |tx| {
            Device::enrol_usb(tx, &mut enrolled).await?;
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

    let payload: DeviceListItem = DeviceListItem::from(&*device);
    let _ = app.emit("device_connected", payload);

    let serial = &device.serial;
    with_loading(app, async {
        let imported = import_from_device(app, serial).await.unwrap_or(0);

        download_pending_workouts(app, serial).await;
        delete_pending_workouts(app, serial).await;
        push_pending_workouts(app, serial).await;
        // After the workouts, so the device already has the ones the schedule points to
        send_schedule_to_device(app, serial, false).await;

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

/// Serial numbers of the devices plugged in by USB; none if they can't be listed, which is logged.
async fn connected_serials<R: Runtime>(app: &AppHandle<R>) -> Vec<String> {
    let mtp_client = app.state::<MtpLock>();
    // Bound in its own statement so the MTP lock is released before the caller uses the devices
    let connected = mtp_client.lock().await.get_connected_devices().await;
    match connected {
        Ok(devices) => devices.into_iter().map(|d| d.serial).collect(),
        Err(e) => {
            error!("Error listing connected devices: {e}");
            Vec::new()
        }
    }
}

/// Tells the user the workout was sent to a device.
pub(crate) fn notify_workout_sent(workout: &str, serial: &str, lang: Languages) {
    info!("Workout '{workout}' sent to device {serial}");
    show_notification(NotificationDefinition {
        title: workout.to_string(),
        body: translate("ok_workout_send", lang),
        kind: NotificationKind::Temporal,
    });
}

/// Tells the user the workout will be sent to a device the next time it is plugged in.
pub(super) fn notify_workout_pending(workout: &str, serial: &str, lang: Languages) {
    info!("Workout '{workout}' pending to be sent to device {serial}");
    show_notification(NotificationDefinition {
        title: workout.to_string(),
        body: translate("pending_workout_send", lang),
        kind: NotificationKind::Temporal,
    });
}

/// Uploads the pending workouts of every device plugged in by USB. The rest keep them pending for
/// the next plug: Connect only gets the workout itself, never a delivery to a device.
pub(crate) async fn push_pending_to_usb<R: Runtime>(app: &AppHandle<R>) {
    for serial in connected_serials(app).await {
        push_pending_workouts(app, &serial).await;
    }
}

/// Delivers a just enabled `workout` through Connect to the linked devices still waiting for it,
/// so call it after `push_pending_to_usb` for USB to take precedence.
pub(crate) async fn send_enabled_workout_to_connect_devices<R: Runtime>(
    app: &AppHandle<R>,
    id: u16,
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
    let serials = match pool(app)
        .run_in_connection(async move |conn| {
            Ok(WorkoutDevice::get_pending_connect_devices(conn, id).await?)
        })
        .await
    {
        Ok(serials) => serials,
        Err(e) => {
            error!("Error fetching Connect devices waiting for workout {id}: {e}");
            return;
        }
    };
    if serials.is_empty() {
        return;
    }

    let workout = workout_name(app, id).await;
    let lang = language(app);
    for serial in serials {
        if let Err(e) = queue_workout_on_connect(app, id, &serial).await {
            error!("Error pushing pending workout '{workout}' to {serial}: {e}");
            continue;
        }
        finish_push(app, &workout, id, &serial, lang).await;
    }
}

/// Delete the pending workouts of every currently connected device.
pub(crate) async fn delete_pending_from_connected<R: Runtime>(app: &AppHandle<R>) {
    for serial in connected_serials(app).await {
        delete_pending_workouts(app, &serial).await;
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

    let pending = match pool(app)
        .run_in_connection(async move |conn| {
            Ok(WorkoutDevice::get_pending(conn, &serial_q, Some(true)).await?)
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

    let (lang, weight_unit) = fit_settings(app);
    for (id, workout) in pending {
        if let Err(e) = send_workout_to_device(app, lang, weight_unit, id, &serial).await {
            error!("Error pushing pending workout '{workout}' to {serial}: {e}");
            continue;
        }
        finish_push(app, &workout, id, &serial, lang).await;
    }
}

/// Removes the queue entry of a workout just sent to the device `serial` and notifies it.
pub(super) async fn finish_push<R: Runtime>(
    app: &AppHandle<R>,
    workout: &str,
    id: u16,
    serial: &str,
    lang: Languages,
) {
    clear_workout_pending(app, workout, id, serial).await;
    notify_workout_sent(workout, serial, lang);
}

/// Deletes from `device` the disabled workouts queued while it was disconnected, removing each queue entry once deleted.
pub(super) async fn delete_pending_workouts<R: Runtime>(app: &AppHandle<R>, serial: &str) {
    // Held for the whole run: a waiting caller re-reads the queue after this one has cleared it
    let _guard = PUSH_LOCK.lock().await;
    let serial = serial.to_string();
    let serial_q = serial.clone();
    let pending = match pool(app)
        .run_in_connection(async move |conn| {
            Ok(WorkoutDevice::get_pending(conn, &serial_q, Some(false)).await?)
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

    let lang = language(app);
    for (id, workout) in pending {
        // A workout renamed since sent is still under its old name on the device
        if let Err(e) = delete_previous_file(app, id, &serial).await {
            error!("Error deleting previous file of workout '{workout}' from {serial}: {e}");
            continue;
        }
        match delete_workout_from_device(app, &workout, &serial).await {
            Ok(deleted) => {
                clear_workout_pending(app, &workout, id, &serial).await;
                if deleted {
                    info!("Workout '{workout}' deleted from device {serial}");
                    show_notification(NotificationDefinition {
                        title: workout.clone(),
                        body: translate("ok_workout_delete", lang),
                        kind: NotificationKind::Temporal,
                    });
                }
            }
            Err(e) => {
                error!("Error deleting workout '{workout}' from {serial}: {e}");
            }
        }
    }
}

/// Stores the steps of the `pending_workouts` found among the `.fit` files fetched from the
/// device. Those that fail to parse or to be stored are kept as pending files.
pub(super) async fn store_pending_workouts<R: Runtime>(
    app: &AppHandle<R>,
    files: &[FitFile],
    serial: &str,
    pending_workouts: &[String],
) {
    // Every workout on the device gets downloaded; those not pending are just discarded
    let mut failed = Vec::new();
    let mut workouts_to_insert = Vec::new();
    // Parsing is CPU-bound and grows with the workouts on the device
    let to_parse = files.to_vec();
    let parsed = tokio::task::spawn_blocking(move || {
        to_parse
            .into_iter()
            .map(|file| Workout::try_from(FitParser::from_fit_file(file)))
            .collect::<Vec<_>>()
    })
    .await
    .expect("FIT parsing task panicked");
    for (file, parsed) in files.iter().zip(parsed) {
        match parsed {
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

    let mut workout_ids: Vec<u16> = Vec::new();
    for (mut workout, file) in workouts_to_insert {
        let name = workout.name.clone();
        let res = pool(app)
            .run_in_transaction(async move |tx| {
                Workout::store_steps(tx, &mut workout).await?;
                Ok(workout.id)
            })
            .await;

        match res {
            Ok(id) => workout_ids.push(id),
            Err(e) => {
                error!("Error storing workout {} from {}: {}", name, serial, e);
                failed.push(file);
            }
        }
    }

    store_pending_files(
        &constants::PENDING_DOWNLOAD_WORKOUTS_FOLDER,
        serial,
        &failed,
    )
    .await;

    if !workout_ids.is_empty() {
        let _ = app.emit("added_workout_steps", workout_ids);
    }
}

/// Downloads from `device` the step definitions of workouts that have sessions recorded on it but no steps stored yet.
pub(super) async fn download_pending_workouts<R: Runtime>(app: &AppHandle<R>, serial: &str) {
    let serial_q = serial.to_string();
    let pending_workouts = match pool(app)
        .run_in_connection(async move |conn| Ok(Workout::get_without_steps(conn, &serial_q).await?))
        .await
    {
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
            store_pending_workouts(app, &files, serial, &pending_workouts).await;
        }
        Err(e) => error!("Error downloading workouts from {}: {e}", serial),
    }
}

pub(super) async fn send_workout_to_device<R: Runtime>(
    app: &AppHandle<R>,
    lang: Languages,
    weight_unit: WeightUnit,
    id: u16,
    serial: &str,
) -> Result<(), String> {
    let workout = pool(app)
        .run_in_connection(async move |conn| Ok(Workout::get_for_device(conn, id).await?))
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("Workout {id} not found"))?;

    // The device would ignore the renamed workout, or keep both, next to the old file
    delete_previous_file(app, id, serial).await?;

    // File name is kept as-is since it becomes the workout file name on the device
    let file_name = workout.get_workout_file_name();
    upload_fit(app, serial, &file_name, move || {
        FitWriter::from(&workout).into_bytes(lang, weight_unit)
    })
    .await
}

/// Encodes a FIT file in memory with `encode` and uploads it as `file_name` to `GARMIN/NewFiles` on
/// the device `serial`, from where the device validates and imports it once unplugged.
async fn upload_fit<R: Runtime, E: ToString>(
    app: &AppHandle<R>,
    serial: &str,
    file_name: &str,
    encode: impl FnOnce() -> Result<Vec<u8>, E> + Send + 'static,
) -> Result<(), String> {
    // Encoding is CPU-bound work
    let data = tokio::task::spawn_blocking(move || encode().map_err(|e| e.to_string()))
        .await
        .expect("FIT encoding task panicked")?;

    app.state::<MtpLock>()
        .lock()
        .await
        .upload_file(
            serial,
            constants::MTP_GARMIN_NEW_FILES_FOLDER,
            file_name,
            data,
        )
        .await
        .map_err(|e| e.to_string())
}

/// Deletes from the device `serial` the file the workout `id` had there before being renamed, if
/// any, and forgets that name.
async fn delete_previous_file<R: Runtime>(
    app: &AppHandle<R>,
    id: u16,
    serial: &str,
) -> Result<(), String> {
    let device = serial.to_string();
    let previous = pool(app)
        .run_in_connection(async move |conn| {
            Ok(WorkoutDevice::get_previous_name(conn, id, &device).await?)
        })
        .await
        .map_err(|e| e.to_string())?;
    let Some(previous) = previous else {
        return Ok(());
    };

    if delete_workout_from_device(app, &previous, serial).await? {
        info!("Deleted file of previous workout name '{previous}' from device {serial}");
    }
    let device = serial.to_string();
    pool(app)
        .run_in_connection(async move |conn| {
            Ok(WorkoutDevice::clear_previous_name(conn, id, &device).await?)
        })
        .await
        .map_err(|e| e.to_string())
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

/// Serializes schedule uploads so an older schedule can't land on a device after a newer one.
static SCHEDULE_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// Sends the schedule of the next days to every device plugged in by USB, even if it has no plans
/// left, so removed plans leave the devices too.
pub(crate) async fn send_schedule_to_connected<R: Runtime>(app: &AppHandle<R>) {
    for serial in connected_serials(app).await {
        send_schedule_to_device(app, &serial, true).await;
    }
}

/// Uploads by USB to the device `serial` the workouts planned for the next
/// [`constants::SCHEDULE_DAYS`] days, today included. Without plans nothing is sent unless
/// `send_empty`. Failures are only logged.
pub(super) async fn send_schedule_to_device<R: Runtime>(
    app: &AppHandle<R>,
    serial: &str,
    send_empty: bool,
) {
    // Held from reading the plans to the upload, so the last change is the last one sent
    let _guard = SCHEDULE_LOCK.lock().await;

    let Some(from) = today_local_midnight() else {
        error!("Could not compute the start of today");
        return;
    };
    let Some(to) = local_midnight(i64::from(from) + constants::SCHEDULE_DAYS * 86_400 + 43_200)
    else {
        error!("Could not compute the end of the schedule window");
        return;
    };

    let plans = match pool(app)
        .run_in_connection(
            async move |conn| Ok(WorkoutPlanning::get_between(conn, from, to).await?),
        )
        .await
    {
        Ok(plans) => plans,
        Err(e) => {
            error!("Error fetching the planned workouts for {serial}: {e}");
            return;
        }
    };
    if plans.is_empty() && !send_empty {
        return;
    }

    upload_missing_workouts(app, serial, &plans).await;

    let count = plans.len();
    match upload_schedule(app, serial, plans).await {
        Ok(()) => info!("Schedule with {count} planned workouts sent to device {serial}"),
        Err(e) => error!("Error sending the schedule to device {serial}: {e}"),
    }
}

/// Uploads to the device `serial` the workouts of `plans` it lacks, so the schedule can point to
/// them, and records them in `workout_device` as sent. A workout counts as on the device when it
/// has an entry there not pending. Only the workouts the schedule will hold are checked. Failures
/// are only logged.
async fn upload_missing_workouts<R: Runtime>(
    app: &AppHandle<R>,
    serial: &str,
    plans: &[WorkoutPlanning],
) {
    let serial_cl = serial.to_string();
    let on_device = match pool(app)
        .run_in_connection(async move |conn| Ok(WorkoutDevice::get_sent(conn, &serial_cl).await?))
        .await
    {
        Ok(names) => names,
        Err(e) => {
            error!("Error fetching the workouts sent to {serial}: {e}");
            return;
        }
    };
    let mut missing: Vec<(u16, &str)> = plans
        .iter()
        .filter_map(|p| p.workout_obj.as_ref())
        .filter(|w| w.file_created().is_some() && !on_device.contains(&w.id))
        .map(|w| (w.id, w.name.as_str()))
        .collect();
    missing.sort_unstable();
    missing.dedup();
    if missing.is_empty() {
        return;
    }

    let (lang, weight_unit) = fit_settings(app);
    for (id, workout) in missing {
        info!("Planned workout '{workout}' missing on {serial}, uploading it");
        if let Err(e) = send_workout_to_device(app, lang, weight_unit, id, serial).await {
            error!("Error uploading planned workout '{workout}' to {serial}: {e}");
            continue;
        }

        if let Err(e) = set_workout_on_device(app, id, serial, false).await {
            error!("Error registering workout '{workout}' for device {serial}: {e}");
        }
    }
}

/// Writes `plans` as a schedule file and uploads it to the device `serial`.
async fn upload_schedule<R: Runtime>(
    app: &AppHandle<R>,
    serial: &str,
    plans: Vec<WorkoutPlanning>,
) -> Result<(), String> {
    let (lang, weight_unit) = fit_settings(app);
    upload_fit(
        app,
        serial,
        constants::MTP_GARMIN_SCHEDULE_FILE,
        move || FitWriter::from(&plans).into_bytes(lang, weight_unit),
    )
    .await
}

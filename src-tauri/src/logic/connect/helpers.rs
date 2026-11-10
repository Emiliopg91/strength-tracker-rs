use tauri::{AppHandle, Emitter, Manager, Runtime};
use tauri_plugin_log::log::{error, info};

use super::queries::{
    pending_connect_workouts, session_sets_for_connect, set_connect_pending, sync_connect_devices,
    workout_connect_id, workout_with_exercises,
};
use crate::{
    ConnectLock, SettingsLock,
    connect::ConnectClient,
    dao::device::Device,
    dto::{
        connect::{ConnectUpdateSets, ConnectWorkout},
        devices::DeviceListItem,
        sessions::SessionStepDto,
    },
    logic::{report_error, run_blocking, run_blocking_conn, run_blocking_tx},
};

/// Uploads `workout` to Connect, notifying the user if it fails.
/// The workout is left pending of upload on failure and cleared on success.
pub(super) async fn send_workout_connect<R: Runtime>(
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
pub(crate) async fn sync_workouts_connect<R: Runtime>(
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
pub(crate) async fn up_to_date_connect<R: Runtime>(app: &AppHandle<R>) {
    let connect = app.state::<ConnectLock>();
    if connect.lock().await.logged_user().is_none() {
        return;
    }

    let pending = match run_blocking_conn(app, move |_, conn| pending_connect_workouts(conn)).await
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

pub(super) async fn mark_connect_pending<R: Runtime>(
    app: &AppHandle<R>,
    workouts: &[String],
    pending: bool,
) {
    let workouts = workouts.to_vec();
    let res = run_blocking_conn(app, move |_, conn| {
        set_connect_pending(conn, &workouts, pending)
    })
    .await;

    if let Err(e) = res {
        error!("Error updating Connect upload status: {e}");
    }
}

pub(super) async fn upload_workout_connect<R: Runtime>(
    app: &AppHandle<R>,
    connect: &ConnectLock,
    settings: &SettingsLock,
    workout: &str,
) -> Result<(), String> {
    let weight_unit = settings.read().unwrap().weight_unit;

    let id = workout.to_string();
    let mut workout = run_blocking_conn(app, move |_, conn| workout_with_exercises(conn, &id))
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

/// Mirrors the workout-capable devices of the Connect account into the database: enrolls new
/// ones, refreshes the Connect-owned fields of known ones and clears the `connect` flag of those
/// no longer listed. Emits `device_discovered` per added or changed device
/// Does nothing if the device list cannot be fetched.
pub(crate) async fn get_devices_from_connect<R: Runtime>(app_orig: &AppHandle<R>) {
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

    let res = run_blocking_tx(app_orig, move |_, tx| sync_connect_devices(tx, &listed)).await;

    match res {
        Ok(discovered) => {
            for device in discovered {
                // Typed binding: the TypeScript generator reads the payload type from it
                let payload: DeviceListItem = device;
                let _ = app_orig.emit("device_discovered", payload);
            }
        }
        Err(e) => error!("Error syncing Connect devices: {e}"),
    }
}

/// Queues `workout` in Connect for delivery to the device `serial`, first uploading the workout
/// to Connect if it has no Connect id yet.
pub(crate) async fn queue_workout_on_connect<R: Runtime>(
    app: &AppHandle<R>,
    workout: &str,
    serial: &str,
) -> Result<(), String> {
    let fetch_connect_id = || {
        let name = workout.to_string();
        run_blocking(app, move |database| workout_connect_id(database, &name))
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

/// Mirrors the user edits of a session's sets on its Connect activity, if the session is linked to
/// one and a user is logged in. Runs after the local save, so failures are only logged.
pub(crate) async fn sync_session_sets_connect<R: Runtime>(
    app: &AppHandle<R>,
    timestamp: u32,
    sets: Vec<SessionStepDto>,
) {
    let res = run_blocking_conn(app, move |_, conn| {
        session_sets_for_connect(conn, timestamp, &sets)
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
pub(super) async fn update_session_sets(
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
            // Generic exercises go without name, just with their category
            name: Some(set.exercise_id.clone()).filter(|name| !name.is_empty()),
            probability: 100_f64,
        }];
    }

    let _ = connect.update_session_sets(&original_sets).await;
}

use std::collections::{BTreeSet, HashMap};

use tauri::{AppHandle, Emitter, Manager, Runtime};
use tauri_plugin_log::log::{error, info};

use crate::{
    ConnectLock, SettingsLock,
    connect::ConnectClient,
    dao::{
        device::Device, exercise::Exercise, session::Session, workout::Workout,
        workout_device::WorkoutDevice, workout_planning::WorkoutPlanning,
    },
    dto::{
        connect::{ConnectUpdateSets, ConnectWorkout},
        devices::DeviceListItem,
        sessions::SessionStepDto,
    },
    logic::{pool, report_error},
    utils::constants,
};

/// Uploads `workout` to Connect, notifying the user if it fails.
/// The workout is left pending of upload on failure and cleared on success.
pub async fn send_workout_connect<R: Runtime>(
    app: &AppHandle<R>,
    connect: &ConnectLock,
    settings: &SettingsLock,
    workout: &str,
) -> Result<String, String> {
    let res = upload_workout_connect(app, connect, settings, workout).await;
    mark_connect_pending(app, &[workout.to_string()], res.is_err()).await;
    res.map_err(|e| {
        report_error(
            format!("{workout}: {e}"),
            settings,
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

    align_workouts_connect(app, &connect).await;

    let (pending_workouts, pending_plans) = match pool(app)
        .run_in_connection(async move |conn| {
            let wkts =
                WorkoutDevice::get_pending(conn, constants::CONNECT_DEVICE_SERIAL, None).await?;
            let plans = WorkoutPlanning::get_pending_connect(conn).await?;
            Ok((wkts, plans))
        })
        .await
    {
        Ok((pending_workouts, pending_plans)) => (pending_workouts, pending_plans),
        Err(e) => {
            error!("Error fetching workouts pending of upload to Connect: {e}");
            return;
        }
    };

    if !pending_workouts.is_empty() {
        info!(
            "Uploading {} pending workouts to Connect",
            pending_workouts.len()
        );
        let settings = app.state::<SettingsLock>();
        for workout in pending_workouts {
            let res = upload_workout_connect(app, &connect, &settings, &workout).await;
            if let Err(e) = &res {
                error!("Error uploading pending workout '{workout}' to Connect: {e}");
            }
            mark_connect_pending(app, std::slice::from_ref(&workout), res.is_err()).await;
        }
    }

    if !pending_plans.is_empty() {
        info!("Uploading {} pending plans to Connect", pending_plans.len());
        for (date, workout) in pending_plans {
            push_plan_to_connect(app, date, &workout).await;
        }
    }
}

/// Copies into the database the Connect creation time of the workouts already uploaded.
/// Failures are only logged, as the next alignment or upload retries them.
async fn align_workouts_connect<R: Runtime>(app: &AppHandle<R>, connect: &ConnectLock) {
    let listed = connect.lock().await.list_workouts().await;
    let listed = match listed {
        Ok(listed) => listed,
        Err(e) => {
            error!("Error listing Connect workouts to align them: {e}");
            return;
        }
    };

    let created_times: Vec<(String, u32)> = listed
        .into_iter()
        .filter_map(|w| w.created_date.map(|c| (w.workout_id.to_string(), c)))
        .collect();
    let res = pool(app)
        .run_in_transaction(async move |tx| {
            Ok(Workout::set_connect_created_times(tx, &created_times).await?)
        })
        .await;

    match res {
        Ok(0) => {}
        Ok(changed) => info!("Aligned the Connect creation time of {changed} workouts"),
        Err(e) => error!("Error storing the Connect creation time of workouts: {e}"),
    }
}

pub(super) async fn mark_connect_pending<R: Runtime>(
    app: &AppHandle<R>,
    workouts: &[String],
    pending: bool,
) {
    let workouts = workouts.to_vec();
    let res = pool(app)
        .run_in_connection(async move |conn| {
            Ok(WorkoutDevice::set_connect_pending(conn, &workouts, pending).await?)
        })
        .await;

    if let Err(e) = res {
        error!("Error updating Connect upload status: {e}");
    }
}

pub async fn upload_workout_connect<R: Runtime>(
    app: &AppHandle<R>,
    connect: &ConnectLock,
    settings: &SettingsLock,
    workout: &str,
) -> Result<String, String> {
    let weight_unit = settings.read().unwrap().weight_unit;

    let id = workout.to_string();
    let mut workout = pool(app)
        .run_in_connection(async move |conn| Ok(Workout::get_with_exercises(conn, &id).await?))
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("Workout {workout} not found"))?;

    let mut client = connect.lock().await;
    let original_id = workout.connect_id.clone();
    let original_created = workout.connect_created_time;

    // The listing gives the id of a workout already in Connect under this name, and the creation
    // time of one already uploaded
    if workout.connect_id.is_none() || workout.connect_created_time.is_none() {
        let listed = client.list_workouts().await.map_err(|e| e.to_string())?;
        let found = match workout.connect_id.as_deref() {
            Some(id) => listed.into_iter().find(|w| w.workout_id.to_string() == id),
            None => listed.into_iter().find(|w| w.workout_name == workout.name),
        };
        if let Some(found) = found {
            workout.connect_id = Some(found.workout_id.to_string());
            workout.connect_created_time = found.created_date;
        }
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
            workout.connect_created_time = created.created_date;
        }
    }
    drop(client);

    let connect_id = workout.connect_id.clone().unwrap();
    if workout.connect_id != original_id || workout.connect_created_time != original_created {
        pool(app)
            .run_in_transaction(async move |tx| Ok(workout.set_connect_keys(tx).await?))
            .await
            .map_err(|e| e.to_string())?;
    }

    Ok(connect_id)
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

    let res = pool(app_orig)
        .run_in_transaction(async move |tx| {
            Ok(Device::sync_connect(tx, &listed)
                .await?
                .iter()
                .map(DeviceListItem::from)
                .collect::<Vec<_>>())
        })
        .await;

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
    let fetch_connect_id = async |workout: String| {
        pool(app)
            .run_in_connection(async move |conn| Ok(Workout::get_connect_id(conn, &workout).await?))
            .await
    };

    let connect_id = match fetch_connect_id(workout.to_string())
        .await
        .map_err(|e| e.to_string())?
    {
        Some(id) => id,
        None => {
            send_workout_connect(
                app,
                &app.state::<ConnectLock>(),
                &app.state::<SettingsLock>(),
                workout,
            )
            .await?;
            fetch_connect_id(workout.to_string())
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
    let res = pool(app)
        .run_in_connection(async move |conn| {
            let Some(connect_id) = Session::get_connect_id(conn, timestamp).await? else {
                return Ok(None);
            };

            let keys: BTreeSet<(u16, u16)> =
                sets.iter().filter_map(|s| s.ex_cat.zip(s.ex_id)).collect();
            let names: HashMap<(u16, u16), (String, String)> =
                Exercise::get_by_keys_with_category(conn, &keys)
                    .await?
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

pub async fn push_plan_to_connect<R: Runtime>(app: &AppHandle<R>, date: u32, workout: &str) {
    let connect_state = app.state::<ConnectLock>();
    let connect = connect_state.lock().await;
    if connect.logged_user().is_some() {
        drop(connect);
        let name = workout.to_string();
        let workout_id = match pool(app)
            .run_in_connection(async move |conn| Ok(Workout::get(conn, &name).await?))
            .await
        {
            Err(e) => {
                error!("Could not get workout entry {}: {}", workout, e);
                None
            }
            Ok(None) => None,
            Ok(Some(workout)) => {
                if workout.connect_id.is_none() {
                    send_workout_connect(
                        app,
                        &app.state::<ConnectLock>(),
                        &app.state::<SettingsLock>(),
                        &workout.name,
                    )
                    .await
                    .ok()
                } else {
                    workout.connect_id.clone()
                }
            }
        };

        let Some(workout_id) = workout_id else {
            error!("Could not push workout to Garmin Connect");
            return;
        };

        let mut connect = connect_state.lock().await;
        match connect
            .schedule_workout(workout_id.parse().unwrap(), date)
            .await
        {
            Ok(schedule_id) => {
                let workout = workout.to_string();
                let res = pool(app)
                    .run_in_connection(async move |conn| {
                        Ok(WorkoutPlanning::set_schedule_id(
                            conn,
                            date,
                            &workout,
                            &schedule_id.to_string(),
                        )
                        .await?)
                    })
                    .await;

                if let Err(e) = res {
                    report_error(
                        e,
                        &app.state::<SettingsLock>(),
                        "error_workout_schedule_connect",
                        "Could not store Garmin Connect schedule id",
                    );
                } else {
                    info!("Planning sent to Connect")
                }
            }
            Err(e) => {
                report_error(
                    e,
                    &app.state::<SettingsLock>(),
                    "error_workout_schedule_connect",
                    "Could not schedule workout in Garmin Connect",
                );
            }
        }
    }
}

pub async fn delete_plan_from_connect<R: Runtime>(app: &AppHandle<R>, schedule_id: u64) {
    let connect_state = app.state::<ConnectLock>();
    let mut connect = connect_state.lock().await;

    if let Err(e) = connect.delete_workout_schedule(schedule_id).await {
        report_error(
            e,
            &app.state::<SettingsLock>(),
            "error_workout_unschedule_connect",
            "Could not remove scheduled workout from Garmin Connect",
        );
    }
}

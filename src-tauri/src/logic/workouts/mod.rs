pub mod commands;
pub mod helpers;

use std::collections::{HashMap, HashSet};

use tauri::{AppHandle, Emitter, Runtime, WebviewWindow};
use tauri_plugin_log::log::info;

use self::helpers::{workout_details, workout_list};
use crate::{
    ConnectLock, SettingsLock,
    dao::{
        exercise::Exercise,
        workout::Workout,
        workout_planning::WorkoutPlanning,
        workout_step::{WorkoutStep, current_one_rms},
    },
    dto::{
        notifications::{NotificationDefinition, NotificationKind},
        workouts::{PercentLoadBase, WorkoutDetails, WorkoutListItem, WorkoutPlanListItem},
    },
    logic::{
        connect::helpers::{delete_plan_from_connect, push_plan_to_connect, sync_workouts_connect},
        devices::helpers::{
            delete_pending_from_connected, push_pending_to_usb,
            send_enabled_workout_to_connect_devices, send_schedule_to_connected,
        },
        exercises::helpers::progression_suggestions,
        notifications::show_notification,
        open_window, pool, print_to_pdf, report_error, settings_language,
    },
    utils::{
        time::{now, today_local_midnight},
        translations::{translate, translate_and_replace},
    },
};

/// Returns sessions grouped/aggregated by workout name (count, average time, latest date), sorted by name.
pub async fn get_workout_list<R: Runtime>(
    app: &AppHandle<R>,
    settings: &SettingsLock,
) -> Result<Vec<WorkoutListItem>, String> {
    info!("Getting workouts list...");
    pool(app)
        .run_in_connection(async move |conn| Ok(workout_list(conn).await?))
        .await
        .inspect(|l| info!("Retreived {} workouts", l.len()))
        .map_err(|e| {
            report_error(
                e,
                settings,
                "error_workout_list",
                "Error getting workouts list",
            )
        })
}

/// Returns the workout `id` with every session following it and its per-session volume.
pub async fn get_workout_details<R: Runtime>(
    app: &AppHandle<R>,
    settings: &SettingsLock,
    id: u16,
) -> Result<WorkoutDetails, String> {
    pool(app)
        .run_in_connection(async move |conn| Ok(workout_details(conn, id).await?))
        .await
        .inspect(|l| info!("Found details for workout {}", l.name))
        .map_err(|e| {
            report_error(
                e,
                settings,
                "error_workout_details",
                "Error getting workout details",
            )
        })
}

pub async fn set_workout_status<R: Runtime>(
    app: &AppHandle<R>,
    workout: u16,
    status: bool,
) -> Result<(), String> {
    let res = pool(app)
        .run_in_transaction(async move |tx| Ok(Workout::set_enabled(tx, workout, status).await?))
        .await
        .map_err(|e| e.to_string());

    if res.is_ok() {
        if status {
            push_pending_to_usb(app).await;
            send_enabled_workout_to_connect_devices(app, workout).await;
        } else {
            delete_pending_from_connected(app).await;
        }
    }

    res
}

pub async fn set_workout_progression<R: Runtime>(
    app: &AppHandle<R>,
    settings: &SettingsLock,
    connect: &ConnectLock,
    workout: u16,
    value: bool,
) -> Result<(), String> {
    let res = pool(app)
        .run_in_transaction(async move |tx| {
            Workout::set_progression(tx, workout, value).await?;
            Ok(!progression_suggestions(tx).await?.is_empty())
        })
        .await;

    match res {
        Ok(any_progression) => {
            info!("Updated progression flag for workout {workout}");
            // Typed binding: the TypeScript generator reads the payload type from it
            let payload: bool = any_progression;
            let _ = app.emit("progressions_available", payload);
            // The inherited exercise notes change with the flag
            sync_workouts_connect(app, connect, settings, &[workout]).await;
            push_pending_to_usb(app).await;
            Ok(())
        }
        Err(e) => Err(report_error(
            e,
            settings,
            "error_workout_save",
            "Error updating progression flag for workout",
        )),
    }
}

pub async fn save_workout<R: Runtime>(
    app: &AppHandle<R>,
    settings: &SettingsLock,
    connect: &ConnectLock,
    edit: bool,
    workout: Workout,
) -> Result<(), String> {
    let name = workout.name.clone();
    let lang = settings_language(settings);

    // Names are unique: a clear message instead of the constraint error
    let lookup = name.clone();
    let owner = pool(app)
        .run_in_connection(async move |conn| Ok(Workout::get_id(conn, &lookup).await?))
        .await
        .map_err(|e| {
            report_error(
                e,
                settings,
                "error_workout_save",
                "Error checking workout name",
            )
        })?;
    if owner.is_some_and(|owner| !edit || owner != workout.id) {
        return Err(report_error(
            translate_and_replace("workout_name_taken", &[&name], lang),
            settings,
            "error_workout_save",
            "Workout name already taken",
        ));
    }

    let res = pool(app)
        .run_in_transaction(async move |tx| {
            let id = Workout::store(tx, &workout, edit).await?;
            // Percentage loads are resolved here: whatever weight the step came with is replaced
            let exercises: HashSet<(u16, u16)> = workout
                .steps
                .iter()
                .filter(|s| s.one_rm_pct.is_some())
                .filter_map(|s| s.ex_cat.zip(s.ex_id))
                .collect();
            let refreshed = WorkoutStep::refresh_percent_loads(tx, Some(&exercises), now()).await?;
            Ok((id, refreshed))
        })
        .await;

    match res {
        Ok((id, refreshed)) => {
            info!("Workout '{}' saved succesfully", name);
            show_notification(NotificationDefinition {
                title: name,
                body: translate("ok_workout_save", lang),
                kind: NotificationKind::Temporal,
            });
            let mut workouts = refreshed;
            if !workouts.contains(&id) {
                workouts.push(id);
            }
            sync_workouts_connect(app, connect, settings, &workouts).await;
            if edit || workouts.len() > 1 {
                push_pending_to_usb(app).await;
            }
            Ok(())
        }
        Err(e) => Err(report_error(
            e,
            settings,
            "error_workout_save",
            "Error saving workout",
        )),
    }
}

/// Current estimated 1RM of every exercise with recent loaded sets.
pub async fn get_percent_load_bases<R: Runtime>(
    app: &AppHandle<R>,
    settings: &SettingsLock,
) -> Result<Vec<PercentLoadBase>, String> {
    pool(app)
        .run_in_connection(async move |conn| {
            let exercises: HashMap<(u16, u16), Exercise> = Exercise::get_all(conn)
                .await?
                .into_iter()
                .map(|e| ((e.category, e.id), e))
                .collect();
            let one_rms = current_one_rms(conn, None, &exercises, now()).await?;
            Ok(one_rms
                .into_iter()
                .map(|((ex_cat, ex_id), one_rm)| PercentLoadBase {
                    ex_cat,
                    ex_id,
                    one_rm,
                })
                .collect())
        })
        .await
        .map_err(|e| {
            report_error(
                e,
                settings,
                "error_workout_details",
                "Error getting the current 1RM estimates",
            )
        })
}

pub async fn load_workout_page<R: Runtime>(app: &AppHandle<R>, workout: u16) -> Result<(), String> {
    open_window(app, "workout", Some(&format!("workout={workout}")))
        .map(|_| ())
        .map_err(|e| e.to_string())
}

/// Called by the workout window once everything is rendered and painted: prints it to PDF.
pub async fn workout_print_ready(window: WebviewWindow, workout: &str) -> Result<(), String> {
    info!("Workout ready, printing");
    print_to_pdf(window, workout).await
}

pub async fn get_planned_workouts<R: Runtime>(
    app: &AppHandle<R>,
) -> Result<Vec<WorkoutPlanListItem>, String> {
    Ok(pool(app)
        .run_in_connection(async |conn| Ok(WorkoutPlanning::get_all(conn).await?))
        .await
        .unwrap_or_default()
        .iter()
        .map(WorkoutPlanListItem::from)
        .collect())
}

pub async fn remove_planned_workout<R: Runtime>(
    app: &AppHandle<R>,
    date: u32,
    workout: u16,
) -> Result<(), String> {
    let res = pool(app)
        .run_in_connection(async move |conn| {
            let entry = WorkoutPlanning::get(conn, date, workout).await?.unwrap();
            // Kept as the record of what was planned: the UI offers no delete for these either
            if entry.session.is_some()
                || today_local_midnight().is_some_and(|today| entry.date < today)
            {
                return Err("Past or done planned workouts can't be deleted".into());
            }
            entry.delete_by_id_in(conn).await?;
            Ok(entry.connect_id)
        })
        .await
        .map_err(|e| e.to_string());

    let connect_id = res?;

    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        send_schedule_to_connected(&app).await;
        if let Some(id) = connect_id {
            delete_plan_from_connect(&app, id.parse().unwrap()).await;
        }
    });

    Ok(())
}

pub async fn add_planned_workout<R: Runtime>(
    app: &AppHandle<R>,
    date: u32,
    workout: u16,
) -> Result<(), String> {
    let res = pool(app)
        .run_in_connection(async move |conn| {
            WorkoutPlanning::insert(conn, date, workout).await?;
            Ok(())
        })
        .await
        .map_err(|e| e.to_string());

    if res.is_ok() {
        let app = app.clone();
        tauri::async_runtime::spawn(async move {
            // Pushing it first can upload the workout, which gives it the Connect creation time
            // the schedule needs
            push_plan_to_connect(&app, date, workout).await;
            send_schedule_to_connected(&app).await;
        });
    }

    res
}

pub async fn get_next_planned_workout(
    app: &AppHandle,
) -> Result<Option<WorkoutPlanListItem>, String> {
    pool(app)
        .run_in_connection(async |conn| {
            Ok(WorkoutPlanning::get_next(conn, today_local_midnight()).await?)
        })
        .await
        .map_err(|e| e.to_string())
        .map(|w| w.map(|w| WorkoutPlanListItem::from(&w)))
}

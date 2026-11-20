pub mod commands;
pub mod helpers;

use tauri::{AppHandle, Emitter, Runtime, WebviewWindow};
use tauri_plugin_log::log::info;

use self::helpers::{workout_details, workout_list};
use crate::{
    ConnectLock, SettingsLock,
    dao::{workout::Workout, workout_planning::WorkoutPlanning},
    dto::{
        notifications::{NotificationDefinition, NotificationKind},
        workouts::{WorkoutDetails, WorkoutListItem, WorkoutPlanListItem},
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
    utils::{time::today_local_midnight, translations::translate},
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
    let res = pool(app)
        .run_in_transaction(async move |tx| Ok(Workout::store(tx, &workout, edit).await?))
        .await;

    let lang = settings_language(settings);
    match res {
        Ok(id) => {
            info!("Workout '{}' saved succesfully", name);
            show_notification(NotificationDefinition {
                title: name,
                body: translate("ok_workout_save", lang),
                kind: NotificationKind::Temporal,
            });
            sync_workouts_connect(app, connect, settings, &[id]).await;
            if edit {
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

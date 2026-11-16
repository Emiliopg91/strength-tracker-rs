pub mod commands;
pub mod queries;

use rusqlite_orm::{
    dao::Repository,
    types::{order_by::OrderBy, where_clause::Where},
};
use tauri::{AppHandle, Runtime, WebviewWindow, WebviewWindowBuilder};
use tauri_plugin_log::log::info;

use self::queries::{set_workout_enabled, store_workout, workout_details, workout_list};
use crate::{
    ConnectLock, SettingsLock,
    dao::{
        workout::Workout,
        workout_planning::{WorkoutPlanningRepository, entity::columns as workout_planning_cols},
    },
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
        notifications::show_notification,
        pool, print_to_pdf, report_error,
        workouts::queries::{fetch_planning, insert_planned_workout},
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

/// Returns every session for one workout name with per-session volume and session-over-session volume diff.
pub async fn get_workout_details<R: Runtime>(
    app: &AppHandle<R>,
    settings: &SettingsLock,
    name: &str,
) -> Result<WorkoutDetails, String> {
    let name = name.to_string();
    pool(app)
        .run_in_connection(async move |conn| Ok(workout_details(conn, &name).await?))
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
    workout: &str,
    status: bool,
) -> Result<(), String> {
    let name = workout;
    let workout = workout.to_string();
    let res = pool(app)
        .run_in_transaction(async move |tx| Ok(set_workout_enabled(tx, &workout, status).await?))
        .await
        .map_err(|e| e.to_string());

    if res.is_ok() {
        if status {
            push_pending_to_usb(app).await;
            send_enabled_workout_to_connect_devices(app, name).await;
        } else {
            delete_pending_from_connected(app).await;
        }
    }

    res
}

pub async fn save_workout<R: Runtime>(
    app: &AppHandle<R>,
    settings: &SettingsLock,
    connect: &ConnectLock,
    edit: bool,
    workout: Workout,
) -> Result<(), String> {
    let name = workout.name.clone();
    let name_cl = workout.name.clone();
    let res = pool(app)
        .run_in_transaction(async move |tx| Ok(store_workout(tx, &workout, edit).await?))
        .await;

    let lang = settings.read().unwrap().language;
    match res {
        Ok(_) => {
            info!("Workout '{}' saved succesfully", name);
            show_notification(NotificationDefinition {
                title: name,
                body: translate("ok_workout_save", lang),
                kind: NotificationKind::Temporal,
            });
            sync_workouts_connect(app, connect, settings, &[name_cl]).await;
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

pub async fn load_workout_page<R: Runtime>(
    app: &AppHandle<R>,
    workout: &str,
) -> Result<(), String> {
    let mut config = app
        .config()
        .app
        .windows
        .iter()
        .find(|w| w.label == "workout")
        .expect("no 'workout' window in tauri.conf.json")
        .clone();

    config.url = tauri::WebviewUrl::App(format!("{}?workout={workout}", config.url).into());

    WebviewWindowBuilder::from_config(app, &config)
        .map_err(|e| e.to_string())?
        .build()
        .map_err(|e| e.to_string())?;

    Ok(())
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
        .run_in_connection(async |conn| Ok(fetch_planning(conn).await?))
        .await
        .unwrap_or_default()
        .iter()
        .map(WorkoutPlanListItem::from)
        .collect())
}

pub async fn remove_planned_workout<R: Runtime>(
    app: &AppHandle<R>,
    date: u32,
    workout: &str,
) -> Result<(), String> {
    let workout = workout.to_string();
    let res = pool(app)
        .run_in_connection(async move |conn| {
            let entry = WorkoutPlanningRepository::select_by_id_in(conn, date, &workout)
                .await?
                .unwrap();
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
    workout: &str,
) -> Result<(), String> {
    let workout = workout.to_string();
    let workout_cl = workout.clone();
    let res = pool(app)
        .run_in_connection(async move |conn| {
            insert_planned_workout(conn, date, workout_cl.as_str()).await?;
            Ok(())
        })
        .await
        .map_err(|e| e.to_string());

    if res.is_ok() {
        let app = app.clone();
        tauri::async_runtime::spawn(async move {
            // Pushing it first can upload the workout, which gives it the Connect creation time
            // the schedule needs
            push_plan_to_connect(&app, date, &workout).await;
            send_schedule_to_connected(&app).await;
        });
    }

    res
}

pub async fn get_next_planned_workout(
    app: &AppHandle,
) -> Result<Option<WorkoutPlanListItem>, String> {
    WorkoutPlanningRepository::select()
        .where_(Where::And(vec![
            Where::Gte(workout_planning_cols::DATE, today_local_midnight().into()),
            Where::Null(workout_planning_cols::SESSION),
        ]))
        .order_by(OrderBy::Asc(workout_planning_cols::DATE))
        .fetch_one(&pool(app))
        .await
        .map_err(|e| e.to_string())
        .map(|w| w.map(|w| WorkoutPlanListItem::from(&w)))
}

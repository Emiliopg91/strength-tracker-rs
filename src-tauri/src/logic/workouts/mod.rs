pub mod commands;
pub mod helpers;
pub mod queries;

use tauri::{AppHandle, Runtime, WebviewWindow, WebviewWindowBuilder};
use tauri_plugin_log::log::info;

use self::helpers::print_to_pdf;
use self::queries::{set_workout_enabled, store_workout, workout_details, workout_list};
use crate::{
    ConnectLock, SettingsLock,
    dao::workout::Workout,
    dto::{
        notifications::{NotificationDefinition, NotificationKind},
        workouts::{WorkoutDetails, WorkoutListItem},
    },
    logic::{
        connect::helpers::sync_workouts_connect,
        devices::helpers::{
            delete_pending_from_connected, push_pending_to_usb,
            send_enabled_workout_to_connect_devices,
        },
        notifications::show_notification,
        report_error, run_blocking_conn, run_blocking_tx,
    },
    utils::translations::translate,
};

/// Returns sessions grouped/aggregated by workout name (count, average time, latest date), sorted by name.
pub async fn get_workout_list<R: Runtime>(
    app: &AppHandle<R>,
    settings: &SettingsLock,
) -> Result<Vec<WorkoutListItem>, String> {
    info!("Getting workouts list...");
    let res = run_blocking_conn(app, move |_, conn| workout_list(conn)).await;
    match res {
        Ok(l) => {
            info!("Retreived {} workouts", l.len());
            Ok(l)
        }
        Err(e) => Err(report_error(
            e,
            settings.read().unwrap().language,
            "error_workout_list",
            "Error getting workouts list",
        )),
    }
}

/// Returns every session for one workout name with per-session volume and session-over-session volume diff.
pub async fn get_workout_details<R: Runtime>(
    app: &AppHandle<R>,
    settings: &SettingsLock,
    name: &str,
) -> Result<WorkoutDetails, String> {
    let name = name.to_string();
    let res = run_blocking_conn(app, move |_, conn| workout_details(conn, &name)).await;

    match res {
        Ok(l) => {
            info!("Found details for workout {}", l.name);
            Ok(l)
        }
        Err(e) => Err(report_error(
            e,
            settings.read().unwrap().language,
            "error_workout_details",
            "Error getting workout details",
        )),
    }
}

pub async fn set_workout_status<R: Runtime>(
    app: &AppHandle<R>,
    workout: &str,
    status: bool,
) -> Result<(), String> {
    let name = workout;
    let workout = workout.to_string();
    let res = run_blocking_tx(app, move |_, tx| set_workout_enabled(tx, &workout, status))
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
    let res = run_blocking_tx(app, move |_, tx| store_workout(tx, &workout, edit)).await;

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
            lang,
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
        .expect("no hay ventana 'workout' en tauri.conf.json")
        .clone();

    config.url = tauri::WebviewUrl::App(format!("{}?workout={workout}", config.url).into());

    WebviewWindowBuilder::from_config(app, &config)
        .map_err(|e| e.to_string())?
        .build()
        .map_err(|e| e.to_string())?;

    Ok(())
}

/// Called by the report window once everything is rendered and painted: prints it to PDF.
pub async fn workout_print_ready(window: WebviewWindow, workout: &str) -> Result<(), String> {
    info!("Report ready, printing");
    let res = print_to_pdf(window.clone(), workout).await;
    // On success the window is closed once the print operation ends
    if res.is_err() {
        let _ = window.close();
    }
    res
}

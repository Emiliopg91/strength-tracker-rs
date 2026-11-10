pub mod commands;
pub mod helpers;
pub mod queries;

use futures_util::TryFutureExt;
use rusqlite_orm::{database::DatabasePool, errors::DatabaseError};
use tauri::{AppHandle, Manager, Runtime};
use tauri_plugin_log::log::info;

use self::helpers::{get_devices_from_connect, queue_workout_on_connect, up_to_date_connect};
use self::queries::{body_metric_dates, connect_import_state, insert_new_body_metrics};
use crate::{
    ConnectLock, SettingsLock,
    connect::client::CalendarItemType,
    dao::workout_device::WorkoutDevice,
    dto::notifications::{NotificationDefinition, NotificationKind},
    logic::{
        devices::queries::queue_workout_device,
        notifications::show_notification,
        report_error, run_blocking,
        sessions::helpers::{import_pending_sessions, update_pending_geolocation},
    },
    utils::translations::translate,
};

pub async fn import_from_connect<R: Runtime>(
    app: &AppHandle<R>,
    settings: &SettingsLock,
    connect: &ConnectLock,
) -> Result<usize, String> {
    let lang = settings.read().unwrap().language;
    let lang_cl = lang;

    let app_cpy = app.clone();
    let res: Result<_, String> = async {
        let (mut device, existing) = run_blocking(app, connect_import_state)
            .map_err(|e: DatabaseError| e.to_string())
            .await?;

        let mut connect = connect.lock().await;

        let calendar_items = connect
            .list_recent_calendar_items()
            .await
            .map_err(|e| e.to_string())?;

        let pending: Vec<u64> = calendar_items
            .iter()
            .filter_map(|e| {
                if e.item_type == CalendarItemType::Activity
                    && let Some(date) = e.start_timestamp_local
                    && !existing.contains(&date)
                {
                    e.id
                } else {
                    None
                }
            })
            .collect();

        let files = connect
            .download_activities(&pending)
            .await
            .map_err(|e| e.to_string())?;

        // The mark only follows what Connect has listed, so activities uploaded late are not missed
        let imported_sessions = tokio::task::spawn_blocking(move || {
            let db = app_cpy.state::<DatabasePool>();
            import_pending_sessions(&app_cpy, &db, &files, &mut device, lang_cl, None)
        })
        .await
        .expect("blocking DB task panicked")
        .map_err(|e: DatabaseError| e.to_string())?;

        Ok(imported_sessions)
    }
    .await;

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

pub async fn import_weight_from_connect<R: Runtime>(
    app: &AppHandle<R>,
    settings: &SettingsLock,
    connect: &ConnectLock,
) -> Result<usize, String> {
    let lang = settings.read().unwrap().language;

    let app_cpy = app.clone();

    let res: Result<usize, String> = async {
        let existing = run_blocking(app, body_metric_dates)
            .map_err(|e: DatabaseError| e.to_string())
            .await?;

        let mut entries = connect
            .lock()
            .await
            .list_weight(existing.first().copied())
            .await
            .map_err(|e| e.to_string())?;

        tokio::task::spawn_blocking(move || {
            let db = app_cpy.state::<DatabasePool>();
            insert_new_body_metrics(&db, &mut entries, &existing)
        })
        .await
        .expect("blocking DB task panicked")
        .map_err(|e: DatabaseError| e.to_string())
    }
    .await;

    match res {
        Ok(0) => {
            info!("No measures added");
            Ok(0)
        }
        Ok(added) => {
            info!("Measures {} added succesfully", added);
            Ok(added)
        }
        Err(e) => Err(report_error(
            e,
            lang,
            "error_adding_body_measures",
            "Error adding measures",
        )),
    }
}

pub async fn connect_logged_user(connect: &ConnectLock) -> Result<Option<String>, String> {
    Ok(connect.lock().await.logged_user())
}

pub async fn connect_login<R: Runtime>(
    app: &AppHandle<R>,
    connect: &ConnectLock,
) -> Result<Option<String>, String> {
    let user = {
        let mut connect = connect.lock().await;
        connect.prompt_login().await.map_err(|e| e.to_string())?;
        connect.logged_user()
    };

    // Released the lock first: the discovery needs it
    if user.is_some() {
        get_devices_from_connect(app).await;
        up_to_date_connect(app).await;
    }

    Ok(user)
}

pub async fn connect_logout(connect: &ConnectLock) -> Result<(), String> {
    connect.lock().await.disconnect().map_err(|e| e.to_string())
}

pub async fn send_to_connect<R: Runtime>(
    app: &AppHandle<R>,
    settings: &SettingsLock,
    workout: &str,
    serial: &str,
) -> Result<(), String> {
    let res: Result<_, Box<dyn std::error::Error>> = async {
        queue_workout_on_connect(app, workout, serial).await?;

        // Register the pair so later changes (e.g. progressions) get re-sent once plugged by USB
        let mut entry = WorkoutDevice {
            device: serial.to_string(),
            workout: workout.to_string(),
            pending: false,
        };
        run_blocking(app, move |database| {
            queue_workout_device(database, &mut entry)
        })
        .await?;

        Ok(())
    }
    .await;

    let lang = settings.read().unwrap().language;

    match res {
        Ok(()) => {
            info!("Workout '{}' sent to device {}", workout, serial);
            show_notification(NotificationDefinition {
                title: workout.to_string(),
                body: translate("ok_workout_send", lang),
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

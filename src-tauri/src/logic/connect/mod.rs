pub mod commands;
pub mod helpers;

use rusqlite_orm::errors::DatabaseError;
use tauri::{AppHandle, Runtime};
use tauri_plugin_log::log::info;

use self::helpers::{get_devices_from_connect, queue_workout_on_connect, up_to_date_connect};
use crate::{
    ConnectLock, SettingsLock,
    connect::client::CalendarItemType,
    dao::{body_metric::BodyMetric, device::Device},
    logic::{
        devices::helpers::{notify_workout_sent, set_workout_on_device},
        pool, report_error,
        sessions::helpers::{finish_session_import, import_pending_sessions},
        settings_language,
    },
};

pub async fn import_from_connect<R: Runtime>(
    app: &AppHandle<R>,
    settings: &SettingsLock,
    connect: &ConnectLock,
) -> Result<usize, String> {
    let lang = settings_language(settings);

    let res: Result<_, String> = async {
        let (mut device, existing) = pool(app)
            .run_in_connection(async |conn| Ok(Device::get_connect_import_state(conn).await?))
            .await
            .map_err(|e| e.to_string())?;

        let mut connect = connect.lock().await;

        let calendar_items = connect
            .list_recent_calendar_items()
            .await
            .map_err(|e| e.to_string())?;

        let pending: Vec<u64> = calendar_items
            .iter()
            .filter(|e| {
                e.item_type == CalendarItemType::Activity
                    && e.start_timestamp_local
                        .is_some_and(|d| !existing.contains(&d))
            })
            .filter_map(|e| e.id)
            .collect();

        let files = connect
            .download_activities(&pending)
            .await
            .map_err(|e| e.to_string())?;

        // The mark only follows what Connect has listed, so activities uploaded late are not
        // missed. Their places are looked up once finished, with the rest of sessions pending it.
        import_pending_sessions(app, files, &mut device, lang, None)
            .await
            .map_err(|e: DatabaseError| e.to_string())
    }
    .await;

    finish_session_import(app, res)
}

pub async fn import_weight_from_connect<R: Runtime>(
    app: &AppHandle<R>,
    settings: &SettingsLock,
    connect: &ConnectLock,
) -> Result<usize, String> {
    let res: Result<usize, String> = async {
        let existing = pool(app)
            .run_in_connection(async |conn| Ok(BodyMetric::get_dates(conn).await?))
            .await
            .map_err(|e| e.to_string())?;

        let mut entries = connect
            .lock()
            .await
            .list_weight(existing.last().copied())
            .await
            .map_err(|e| e.to_string())?;

        if entries.is_empty() {
            Ok(0_usize)
        } else {
            pool(app)
                .run_in_transaction(async move |tx| {
                    Ok(BodyMetric::insert_new(tx, &mut entries, &existing).await?)
                })
                .await
                .map_err(|e| e.to_string())
        }
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
            settings,
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
    connect
        .lock()
        .await
        .disconnect()
        .await
        .map_err(|e| e.to_string())
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
        set_workout_on_device(app, workout, serial, false)
            .await
            .map_err(|e| e.to_string())?;

        Ok(())
    }
    .await;

    let lang = settings_language(settings);

    match res {
        Ok(()) => {
            notify_workout_sent(workout, serial, lang);
            Ok(())
        }
        Err(e) => Err(report_error(
            e,
            settings,
            "error_workout_send",
            "Error sending workout to device",
        )),
    }
}

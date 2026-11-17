pub mod commands;
pub mod helpers;

use std::collections::HashMap;

use rusqlite_orm::errors::DatabaseError;
use tauri::{AppHandle, Runtime};
use tauri_plugin_log::log::{error, info};

use self::helpers::{get_devices_from_connect, queue_workout_on_connect, up_to_date_connect};
use crate::{
    ConnectLock, SettingsLock,
    connect::{ConnectClient, client::CalendarItemType},
    dao::{body_metric::BodyMetric, device::Device, session::Session, session_data::SessionData},
    logic::{
        devices::helpers::{notify_workout_sent, set_workout_on_device},
        pool, report_error,
        sessions::helpers::{finish_session_import, import_pending_sessions},
    },
};

pub async fn import_from_connect<R: Runtime>(
    app: &AppHandle<R>,
    settings: &SettingsLock,
    connect: &ConnectLock,
) -> Result<usize, String> {
    let lang = settings.read().unwrap().language;

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
        let imported_sessions = import_pending_sessions(app, files, &mut device, lang, None)
            .await
            .map_err(|e: DatabaseError| e.to_string())?;

        let ids: HashMap<u32, u64> = calendar_items
            .iter()
            .filter_map(|e| e.start_timestamp_local.zip(e.id))
            .collect();
        name_from_connect_location(app, &mut connect, &imported_sessions, &ids).await;

        Ok(imported_sessions)
    }
    .await;

    finish_session_import(app, res)
}

/// Names the `imported` sessions waiting for geocoding (unnamed, with GPS) after the place Connect
/// gives their activity, found in `ids` by start time, sparing the geocoding query. Those Connect
/// has no place for stay pending. Failures are only logged.
async fn name_from_connect_location<R: Runtime>(
    app: &AppHandle<R>,
    connect: &mut ConnectClient,
    imported: &[u32],
    ids: &HashMap<u32, u64>,
) {
    let pending = match pool(app)
        .run_in_connection(async |conn| Ok(SessionData::get_pending_geolocation(conn).await?))
        .await
    {
        Ok(pending) => pending,
        Err(e) => {
            error!("Error looking for sessions pending geolocation: {e}");
            return;
        }
    };

    for session in pending.iter().map(|p| p.session) {
        let Some(id) = ids.get(&session).filter(|_| imported.contains(&session)) else {
            continue;
        };
        let location = match connect.get_activity_location(*id).await {
            Ok(Some(location)) => location,
            Ok(None) => continue,
            Err(e) => {
                error!("Error fetching the location of activity {id}: {e}");
                continue;
            }
        };
        let name = location.clone();
        let res = pool(app)
            .run_in_connection(async move |conn| Ok(Session::set_name(conn, session, &name).await?))
            .await;
        match res {
            Ok(_) => info!("Session @ {session} named '{location}' after its Connect location"),
            Err(e) => error!("Error naming session {session}: {e}"),
        }
    }
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
            .list_weight(existing.first().copied())
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

    let lang = settings.read().unwrap().language;

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

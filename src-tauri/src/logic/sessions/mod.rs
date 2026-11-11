pub mod commands;
pub mod helpers;
pub mod queries;

use chrono::{Local, TimeZone};
use tauri::{AppHandle, Emitter, Runtime};
use tauri_plugin_log::log::info;

use self::helpers::{
    import_file_paths, notify_new_prs, notify_progression_available, retrieve_session_connect_id,
    session_file_name, write_session_fit,
};
use self::queries::{
    apply_session_changes, fetch_session_details, fetch_sessions, heatmap_data, session_name,
};
use crate::{
    SettingsLock,
    dto::{
        notifications::{NotificationDefinition, NotificationKind},
        sessions::{SessionDetails, SessionListItem, SessionStepsUpdate},
    },
    logic::{
        ask_open_paths, ask_save_path, connect::helpers::sync_session_sets_connect,
        notifications::show_notification, pool, report_error,
    },
    utils::{
        constants,
        translations::{translate, translate_and_replace},
    },
};

/// Returns every recorded session, newest first.
pub async fn get_sessions<R: Runtime>(
    app: &AppHandle<R>,
    settings: &SettingsLock,
    limit: Option<i32>,
) -> Result<Vec<SessionListItem>, String> {
    info!("Getting sessions list...");
    let res = pool(app)
        .run_in_connection(move |conn| Ok(fetch_sessions(conn, limit, None, None, None)?))
        .await;

    match res {
        Ok(l) => {
            info!("Retreived {} sessions", l.len());
            Ok(l)
        }
        Err(e) => Err(report_error(
            e,
            settings.read().unwrap().language,
            "error_session_list",
            "Error getting sessions list",
        )),
    }
}

/// Returns full details for one session (series grouped by exercise, heart rate, GPS, speeds, device).
pub async fn get_session_details<R: Runtime>(
    app: &AppHandle<R>,
    settings: &SettingsLock,
    timestamp: u32,
) -> Result<SessionDetails, String> {
    info!(
        "Getting details for session {}",
        Local
            .timestamp_opt(timestamp as i64, 0)
            .unwrap()
            .to_rfc3339()
    );

    let res = pool(app)
        .run_in_connection(move |conn| Ok(fetch_session_details(conn, timestamp, true, true)?))
        .await;

    match res {
        Ok(details) => {
            info!("Found details for session {}", details.timestamp);
            if details.connect_id.clone().is_none() {
                let app_cl = app.clone();
                tauri::async_runtime::spawn(async move {
                    retrieve_session_connect_id(&app_cl, details.timestamp).await;
                });
            }

            Ok(details)
        }
        Err(e) => Err(report_error(
            e,
            settings.read().unwrap().language,
            "error_session_details",
            "Error getting session details",
        )),
    }
}

/// Asks where to save the session as a FIT file and writes it there.
pub async fn download_session<R: Runtime>(
    app: &AppHandle<R>,
    settings: &SettingsLock,
    timestamp: u32,
) -> Result<(), String> {
    let (lang, weight_unit) = {
        let settings = settings.read().unwrap();
        (settings.language, settings.weight_unit)
    };

    // Without a name the file is just named after the date; a failure surfaces when writing the file
    let name = session_name(&pool(app), timestamp)
        .await
        .ok()
        .flatten()
        .unwrap_or_default();
    let default_file = constants::HOME_DIR.join(session_file_name(timestamp, &name));
    let Some(file) = ask_save_path(
        app,
        &default_file,
        "FIT",
        "fit",
        &translate("select_file_save_fit", lang),
    )
    .await?
    else {
        return Ok(());
    };

    match write_session_fit(app, timestamp, file.clone(), lang, weight_unit).await {
        Ok(()) => {
            show_notification(NotificationDefinition {
                title: translate("ok_session_download", lang),
                body: translate_and_replace(
                    "export_file_path",
                    &[&file.display().to_string()],
                    lang,
                ),
                kind: NotificationKind::Temporal,
            });
            Ok(())
        }
        Err(e) => Err(report_error(
            e,
            lang,
            "error_session_download",
            "Error downloading session",
        )),
    }
}

/// Applies user edits (session notes and reps/weight/exercise of its series) and recomputes personal records.
pub async fn save_session_changes<R: Runtime>(
    app: &AppHandle<R>,
    settings: &SettingsLock,
    details: SessionStepsUpdate,
) -> Result<(), String> {
    info!(
        "Saving changes on session {}...",
        Local
            .timestamp_opt(details.timestamp as i64, 0)
            .unwrap()
            .to_rfc3339()
    );
    let lang = settings.read().unwrap().language;
    let (timestamp, sets) = (details.timestamp, details.sets.clone());

    let res = pool(app)
        .run_in_transaction(move |tx| Ok(apply_session_changes(tx, &details)?))
        .await;

    match res {
        Ok(changes) => {
            info!("Session updated succesfully");
            show_notification(NotificationDefinition {
                title: translate("ok_update_session", lang),
                body: "".to_string(),
                kind: NotificationKind::Temporal,
            });

            if changes.new_prs {
                notify_new_prs(lang);
            }
            if changes.newly_suggested {
                notify_progression_available(lang);
            }
            // Typed binding: the TypeScript generator reads the payload type from it
            let payload: bool = changes.any_progression;
            let _ = app.emit("progressions_available", payload);

            let app_cl = app.clone();
            tauri::async_runtime::spawn(async move {
                sync_session_sets_connect(&app_cl, timestamp, sets).await;
            });

            Ok(())
        }
        Err(e) => Err(report_error(
            e,
            lang,
            "error_update_session",
            "Error updating session",
        )),
    }
}

/// Asks the user for `.fit` files and imports them; returns the number of sessions imported.
pub async fn import_from_files<R: Runtime>(
    app: &AppHandle<R>,
    settings: &SettingsLock,
) -> Result<usize, String> {
    let lang = { settings.read().unwrap().language };
    let files = ask_open_paths(app, "FIT", "fit", &translate("select_files_import", lang)).await?;

    if !files.is_empty() {
        return import_file_paths(app, files).await;
    }

    Ok(0)
}

pub async fn get_heatmap_data<R: Runtime>(
    app: &AppHandle<R>,
) -> Result<Vec<Vec<(u32, u16)>>, String> {
    heatmap_data(&pool(app)).await
}

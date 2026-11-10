use std::{
    path::{Path, PathBuf},
    sync::Mutex,
    time::Duration,
};

use chrono::{Local, TimeZone};
use curl_rest::StatusCode;
use rusqlite_orm::{database::DatabasePool, errors::DatabaseError};
use tauri::{AppHandle, Emitter, Manager, Runtime};
use tauri_plugin_log::log::{error, info};

use super::queries::{
    ImportedFiles, full_session, import_device_files, import_file_list, pending_geolocation,
    set_session_connect_id, set_session_name,
};
use crate::{
    ConnectLock, SettingsLock,
    dao::{device::Device, settings::WeightUnit},
    dto::{
        notifications::{NotificationDefinition, NotificationKind},
        sessions::SessionLocation,
    },
    fit::{FitFile, writer::FitWriter},
    logic::{
        notifications::show_notification, report_error, run_blocking, run_blocking_conn,
        store_pending_files,
    },
    utils::{
        constants,
        translations::{Languages, translate, translate_and_replace},
    },
};

pub(super) async fn retrieve_session_connect_id<R: Runtime>(app: &AppHandle<R>, timestamp: u32) {
    let connect = app.state::<ConnectLock>();
    let mut connect = connect.lock().await;
    if connect.logged_user().is_some()
        && let Ok(Some(id)) = connect.find_session_activity(timestamp).await
    {
        let _ = run_blocking(app, move |database| {
            set_session_connect_id(database, timestamp, id)
        })
        .await;
    }
}

/// Loads one session with its sets (sorted), device, additional data and laps (sorted).
/// File name for an exported session: its local date followed by its name.
pub(crate) fn session_file_name(timestamp: u32, name: &str) -> String {
    let date = Local
        .timestamp_opt(timestamp as i64, 0)
        .unwrap()
        .format("%Y-%m-%d");
    let name = name.trim().replace('/', "-");

    if name.is_empty() {
        format!("{date}.fit")
    } else {
        format!("{date} {name}.fit")
    }
}

/// Encodes the stored session as a FIT file at `path`.
pub(crate) async fn write_session_fit<R: Runtime>(
    app: &AppHandle<R>,
    timestamp: u32,
    path: PathBuf,
    lang: Languages,
    weight_unit: WeightUnit,
) -> Result<(), String> {
    let session = run_blocking_conn(app, move |_, conn| full_session(conn, timestamp))
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("Session {timestamp} not found"))?;

    // Encoding and writing the file is blocking work too
    run_blocking(app, move |_| {
        FitWriter::from(&session)
            .write(path, lang, weight_unit)
            .map_err(|e| e.to_string())
    })
    .await
}

pub(crate) fn import_file_paths<R: Runtime>(
    app: &AppHandle<R>,
    files: &[PathBuf],
) -> Result<usize, String> {
    let lang = app.state::<SettingsLock>().read().unwrap().language;
    let files: Vec<FitFile> = files
        .iter()
        .filter_map(|path| match FitFile::read(path) {
            Ok(file) => Some(file),
            Err(e) => {
                error!("Error reading session file {}: {}", path.display(), e);
                notify_parse_error(&path.display().to_string(), &e.to_string(), lang);
                None
            }
        })
        .collect();
    let database = app.state::<DatabasePool>();
    let res = database
        .run_in_transaction(|tx| Ok(import_file_list(tx, &files)?))
        .map_err(|e| e.to_string());

    match res {
        Ok(imported) => {
            notify_import(app, &imported, lang);
            let res = imported.sessions;
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

/// Imports as sessions of `device` the `.fit` files fetched from it. Those that fail to parse, or
/// all of them if the transaction is rolled back, are stored as pending. With `sync_mark`, the
/// device's last activity sync is moved to it in the same transaction.
pub(crate) fn import_pending_sessions<R: Runtime>(
    app: &AppHandle<R>,
    db: &DatabasePool,
    files: &[FitFile],
    device: &mut Device,
    lang: Languages,
    sync_mark: Option<u32>,
) -> Result<Vec<u32>, DatabaseError> {
    let res = db.run_in_transaction(|tx| Ok(import_device_files(tx, files, device, sync_mark)?));

    let pending_dir = &constants::PENDING_DOWNLOAD_SESSIONS_FOLDER;
    match &res {
        Ok(imported) => {
            notify_import(app, imported, lang);
            store_pending_files(
                pending_dir,
                &device.serial,
                imported.failed.iter().map(|(file, _)| *file),
            );
        }
        Err(_) => store_pending_files(pending_dir, &device.serial, files),
    }

    res.map(|imported| imported.sessions)
}

/// Tells the user, once the import is committed, how it went: files that failed to parse,
/// sessions imported, new records and progressions. Also lets the frontend know whether any
/// progression is pending.
fn notify_import<R: Runtime>(app: &AppHandle<R>, imported: &ImportedFiles, lang: Languages) {
    for (file, e) in &imported.failed {
        notify_parse_error(&file.name, e, lang);
    }

    if !imported.sessions.is_empty() {
        show_notification(NotificationDefinition {
            title: translate("ok_import_sessions", lang),
            body: translate_and_replace(
                "imported_n_sessions",
                &[&imported.sessions.len().to_string()],
                lang,
            ),
            kind: NotificationKind::Temporal,
        });
    }
    if imported.new_prs {
        notify_new_prs(lang);
    }
    if imported.imported_progression {
        notify_progression_available(lang);
    }

    if let Some(any_progression) = imported.any_progression {
        // Typed binding: the TypeScript generator reads the payload type from it
        let payload: bool = any_progression;
        let _ = app.emit("progressions_available", payload);
    }
}

/// Congratulates the user on a new personal record.
pub(super) fn notify_new_prs(lang: Languages) {
    show_notification(NotificationDefinition {
        title: translate("new_record", lang),
        body: translate("contratulations_pr", lang),
        kind: NotificationKind::Temporal,
    });
}

/// Tells the user some exercise is ready to progress.
pub(super) fn notify_progression_available(lang: Languages) {
    show_notification(NotificationDefinition {
        title: translate("progression_available", lang),
        body: translate("progression_available_msg", lang),
        kind: NotificationKind::Temporal,
    });
}

/// Shows the notification of a session file that could not be read or parsed.
fn notify_parse_error(name: &str, error: &str, lang: Languages) {
    let title = Path::new(name)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| name.to_string());
    show_notification(NotificationDefinition {
        title,
        body: translate_and_replace("error_parsing_session", &[error], lang),
        kind: NotificationKind::Persistant,
    });
}

pub(super) static GELOCATION_MUTEX: Mutex<bool> = Mutex::new(false);

/// Recover all pending workouts pending on geolocation
pub(crate) fn update_pending_geolocation<R: Runtime>(app: &AppHandle<R>, db: &DatabasePool) {
    let _lock = GELOCATION_MUTEX.lock().unwrap();
    info!("Looking for pending geocode workouts...");
    let pending = pending_geolocation(db);
    match pending {
        Ok(pendings) => {
            info!("Found {} pending sessions", pendings.len());
            for pending in pendings {
                let first = pending
                    .get_coordinates_degrees()
                    .unwrap()
                    .iter()
                    .find(|e| e.is_some())
                    .unwrap()
                    .unwrap();

                let url = format!(
                    "https://nominatim.openstreetmap.org/reverse?format=json&lat={}&lon={}",
                    first.0, first.1
                );
                //
                let resp = curl_rest::Client::with_user_agent("strength-tracker-rs")
                    .get()
                    .header(curl_rest::Header::Accept("application/json".into()))
                    .send(&url);

                match resp {
                    Ok(response) => {
                        if response.status == StatusCode::Ok {
                            match serde_json::from_slice::<serde_json::Value>(&response.body) {
                                Ok(e) => {
                                    if let Some(address) = e.get("address") {
                                        for key in ["village", "town", "city", "state", "country"] {
                                            if let Some(value) = address.get(key) {
                                                let location = value.to_string().replace("\"", "");

                                                match set_session_name(
                                                    db,
                                                    pending.session,
                                                    &location,
                                                ) {
                                                    Ok(1) => {
                                                        info!(
                                                            "Updated location for session @ {}",
                                                            pending.session
                                                        );
                                                        let payload: SessionLocation =
                                                            SessionLocation {
                                                                session: pending.session,
                                                                location,
                                                            };
                                                        let _ = app.emit(
                                                            "session_location_update",
                                                            payload,
                                                        );
                                                    }
                                                    Ok(_) => {
                                                        info!(
                                                            "Missing session @ {}",
                                                            pending.session
                                                        )
                                                    }
                                                    Err(e) => {
                                                        error!(
                                                            "Error while updating session {}: {}",
                                                            pending.session, e
                                                        )
                                                    }
                                                }

                                                break;
                                            }
                                        }
                                    }
                                }
                                Err(e) => {
                                    error!("Error parsing response: {}", e)
                                }
                            }
                        } else {
                            error!("Invalid status code: {}", response.status)
                        }
                    }
                    Err(e) => {
                        error!("Error on geocode query: {}", e)
                    }
                }

                std::thread::sleep(Duration::from_secs(1));
            }
        }
        Err(e) => {
            error!("Error while looking for pending sessions: {}", e);
        }
    }
}

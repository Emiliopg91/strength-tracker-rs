use std::{
    fs,
    time::{SystemTime, UNIX_EPOCH},
};

use chrono::Local;
use futures_util::TryFutureExt;
use rusqlite_orm::{
    dao::Repository, database::DatabasePool, errors::DatabaseError, types::where_clause::Where,
};
use strength_tracker_rs_macros::traced_command;
use tauri::{AppHandle, Manager, State};
use tauri_plugin_log::log::info;

use crate::{
    SettingsLock,
    connect::CONNECT_CLIENT_INST,
    dao::device::{self, DeviceRepository},
    logic::{
        report_error, run_blocking,
        sessions::{import_file_list, update_pending_geolocation},
    },
    utils::constants,
};

//31-aug-2026 UTC
const MIN_DATE: u32 = 1788134400;

#[traced_command]
#[tauri::command]
pub async fn import_from_connect(
    app: AppHandle,
    settings: State<'_, SettingsLock>,
) -> Result<usize, String> {
    let lang = settings.read().unwrap().language;
    let lang_cl = lang.clone();

    let mut device = run_blocking(app.clone(), |database| {
        Ok(DeviceRepository::select()
            .where_(Where::Eq(device::entity::columns::SERIAL, "connect".into()))
            .fetch_one(database)?
            .unwrap())
    })
    .map_err(|e: DatabaseError| e.to_string())
    .await?;

    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
    let dst_path = std::env::temp_dir().join(format!(
        "{}-{}",
        constants::MTP_TMP_DIR_PREFIX,
        now.as_millis()
    ));
    tokio::fs::create_dir_all(&dst_path)
        .await
        .map_err(|e| e.to_string())?;

    CONNECT_CLIENT_INST
        .lock()
        .await
        .download_activities(device.last_activity_sync.unwrap_or(MIN_DATE), &dst_path)
        .await
        .map_err(|e| e.to_string())?;

    let dst_path_cpy = dst_path.clone();
    let activities = tokio::task::spawn_blocking(move || {
        let mut files = Vec::new();
        if let Ok(read_dir) = fs::read_dir(&dst_path_cpy) {
            for entry in read_dir.flatten() {
                files.push(entry.path());
            }
        };
        files
    })
    .await
    .map_err(|e| e.to_string())?;

    let activities_cpy = activities.clone();
    let app_cpy = app.clone();
    let res: Result<_, _> = tokio::task::spawn_blocking(move || {
        let db = app_cpy.state::<DatabasePool>();
        db.run_in_transaction(move |tx| {
            let res = if !activities_cpy.is_empty() {
                info!("Fetched {} activity files", activities_cpy.len());
                import_file_list(tx, &activities_cpy, Some(device.clone()), lang_cl, true)
            } else {
                Ok(Vec::new())
            }?;

            device.last_activity_sync = Some(Local::now().timestamp() as u32);
            device.update_by_id_in(tx)?;

            Ok(res)
        })
    })
    .await
    .expect("blocking DB task panicked");

    match res {
        Ok(res) => {
            if res.len() == activities.len() {
                let _ = fs::remove_dir_all(dst_path);
            }

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

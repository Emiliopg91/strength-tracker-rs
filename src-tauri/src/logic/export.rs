use std::{
    fs::{self, File},
    io::BufWriter,
};

use chrono::{Local, TimeZone};
use gpx::Gpx;
use rusqlite_orm::database::DatabasePool;
use strength_tracker_rs_macros::traced_command;
use tauri::{AppHandle, Manager, State};
use tauri_plugin_log::log::info;

use crate::{
    SettingsLock,
    dao::session::SessionRepository,
    dto::{
        export::Export,
        notifications::{NotificationDefinition, NotificationKind},
    },
    logic::{notifications::show_notification, report_error, run_blocking},
    rclone::providers::CloudProvider,
    utils::{
        constants,
        translations::{translate, translate_and_replace},
    },
};

#[traced_command]
#[tauri::command]
pub async fn export_gpx(
    app: AppHandle,
    settings: State<'_, SettingsLock>,
    session: u32,
) -> Result<(), String> {
    let session = tokio::task::spawn_blocking(move || {
        let database = app.state::<DatabasePool>();
        database.run_in_connection(|conn| {
            let mut session = SessionRepository::select_by_id_in(conn, session)?.unwrap();
            session.fetch_additional_data_relationship_in(conn)?;
            session.fetch_laps_relationship_in(conn)?;

            Ok(session)
        })
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| e.to_string())?;

    let path = constants::HOME_DIR.join(format!(
        "{}-{}.gpx",
        session.name,
        Local
            .timestamp_millis_opt(session.date as i64 * 1000)
            .unwrap()
    ));
    let path_str = path.display().to_string();
    info!("Exporting track to {}...", path.display());

    let gpx = Gpx::from(session);
    let res = tokio::task::spawn_blocking(move || {
        let file = File::create(&path).map_err(|e| e.to_string())?;
        let writer = BufWriter::new(file);
        gpx::write(&gpx, writer).map_err(|e| e.to_string())?;
        Ok::<(), String>(())
    })
    .await
    .map_err(|e| e.to_string())
    .flatten();

    let lang = settings.read().unwrap().language;
    match res {
        Ok(()) => {
            show_notification(NotificationDefinition {
                title: translate("ok_on_track_export", lang),
                body: translate_and_replace("export_file_path", &[&path_str], lang),
                kind: NotificationKind::Temporal,
            });
            Ok(())
        }
        Err(e) => Err(report_error(
            e,
            lang,
            "error_on_export",
            "Error exporting track",
        )),
    }
}

#[traced_command]
#[tauri::command]
pub async fn upload_to_cloud(
    settings: State<'_, SettingsLock>,
    provider: CloudProvider,
) -> Result<(), String> {
    let lang = settings.read().unwrap().language;

    let res: Result<(), String> = async {
        let configured = provider.is_configured().await.map_err(|e| e.to_string())?;

        if !configured {
            provider.configure().await.map_err(|e| e.to_string())?;
        }

        provider
            .upload(constants::DB_FILE.to_path_buf())
            .await
            .map_err(|e| e.to_string())?;

        Ok(())
    }
    .await;

    match res {
        Ok(l) => {
            info!("File uploaded succesfully");
            show_notification(NotificationDefinition {
                title: translate("upload_succesful", lang),
                body: "".to_string(),
                kind: NotificationKind::Temporal,
            });

            Ok(l)
        }
        Err(e) => Err(report_error(
            e,
            lang,
            "upload_error",
            "Error uploading file",
        )),
    }
}

/// Exports the whole database to a timestamped JSON file in the user's home directory and notifies on success/failure.
#[traced_command]
#[tauri::command]
pub async fn export_database(
    app: AppHandle,
    settings: State<'_, SettingsLock>,
) -> Result<(), String> {
    let path = constants::HOME_DIR.join(format!(
        "{}-{}.json",
        *constants::APP_NAME,
        Local::now().format("%Y-%m-%d-%H-%M-%S")
    ));
    info!("Exporting database to {}...", path.display());

    let export_path = path.clone();
    let export_lang = settings.read().unwrap().language;
    let res = run_blocking(&app, move |database| {
        Export::from_database(database, export_lang)
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
            .and_then(|export| {
                let file = fs::File::create(&export_path)?;
                export.write_json(BufWriter::new(file))?;
                Ok(())
            })
    })
    .await;

    let lang = settings.read().unwrap().language;
    match res {
        Ok(()) => {
            show_notification(NotificationDefinition {
                title: translate("ok_on_export", lang),
                body: translate_and_replace(
                    "export_file_path",
                    &[&path.display().to_string()],
                    lang,
                ),
                kind: NotificationKind::Temporal,
            });
            Ok(())
        }
        Err(e) => Err(report_error(
            e,
            lang,
            "error_on_export",
            "Error exporting database",
        )),
    }
}

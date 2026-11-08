use std::{fs, io::BufWriter};

use chrono::Local;
use tauri::{AppHandle, Runtime};
use tauri_plugin_log::log::info;

use crate::{
    SettingsLock,
    dto::{
        export::Export,
        notifications::{NotificationDefinition, NotificationKind},
    },
    logic::{ask_save_path, notifications::show_notification, report_error, run_blocking},
    rclone::providers::CloudProvider,
    utils::{
        constants,
        translations::{translate, translate_and_replace},
    },
};

pub async fn upload_to_cloud(
    settings: &SettingsLock,
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

/// Asks where to save the whole database as a JSON file, writes it there and notifies on success/failure.
pub async fn export_database<R: Runtime>(
    app: &AppHandle<R>,
    settings: &SettingsLock,
) -> Result<(), String> {
    let default_file = constants::HOME_DIR.join(format!(
        "{}-{}.json",
        *constants::APP_NAME,
        Local::now().format("%Y-%m-%d-%H-%M-%S")
    ));
    let dialog_lang = settings.read().unwrap().language;
    let Some(path) = ask_save_path(
        app,
        &default_file,
        "JSON",
        "json",
        &translate("select_file_save_json", dialog_lang),
    )
    .await?
    else {
        return Ok(());
    };
    info!("Exporting database to {}...", path.display());

    let export_path = path.clone();
    let export_lang = settings.read().unwrap().language;
    let res = run_blocking(app, move |database| {
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

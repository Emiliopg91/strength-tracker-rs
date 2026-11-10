pub mod commands;
pub mod helpers;

use std::collections::HashMap;

use rusqlite_orm::database::DatabasePool;
use tauri::{AppHandle, Manager, Runtime};
use tauri_plugin_autostart::ManagerExt;
use tauri_plugin_log::log::{error, info};

use self::helpers::{check_for_update, install_udev_rules, show_main_window};
use crate::{
    SettingsLock, constants,
    dao::settings::settings_keys::{
        CLOSE_TO_TRAY, DISTANCE_UNIT, LANGUAGE, ON_DEVICE_CONNECT, START_INTO_TRAY, START_ON_BOOT,
        WEIGHT_UNIT,
    },
    dto::{
        app::{AppEnvironment, Settings},
        notifications::{NotificationDefinition, NotificationKind},
    },
    logic::{
        connect::helpers::{get_devices_from_connect, up_to_date_connect},
        devices::helpers::start_device_watcher,
        notifications::show_notification,
        report_error,
        sessions::helpers::update_pending_geolocation,
    },
    utils::translations::{Languages, TRANSLATIONS, translate},
};

/// Returns the current in-memory app settings.
pub async fn get_settings(settings: &SettingsLock) -> Result<Settings, String> {
    Ok(settings.read().unwrap().clone())
}

/// Called once the frontend has mounted: reveals the main window and, without waiting for the
/// network, syncs the Connect devices and then starts the USB device watcher.
pub async fn notify_frontend_ready<R: Runtime>(app: &AppHandle<R>) {
    info!("UI ready");

    let app_cl = app.clone();
    tauri::async_runtime::spawn(async move {
        // Asked here, as the password dialog needs the frontend
        if !std::fs::exists(constants::RULE_FILE).unwrap_or(false)
            && let Err(e) = install_udev_rules(&app_cl, false).await
        {
            error!("Error installing udev rules: {e}");
            app_cl.exit(constants::ExitCodes::UdevError.into());
            return;
        }

        get_devices_from_connect(&app_cl).await;
        start_device_watcher(&app_cl);
        up_to_date_connect(&app_cl).await;
    });

    if !app.state::<SettingsLock>().read().unwrap().start_into_tray {
        let _ = show_main_window(app);
    }

    let app_cl = app.clone();
    std::thread::spawn(move || {
        let db = app_cl.state::<DatabasePool>();
        update_pending_geolocation(&app_cl, &db);
    });

    let app_cl = app.clone();
    std::thread::spawn(move || check_for_update(&app_cl));
}

/// Reports whether this is a debug or release build.
pub async fn get_environment() -> AppEnvironment {
    if cfg!(debug_assertions) {
        AppEnvironment::Debug
    } else {
        AppEnvironment::Release
    }
}

/// Updates a single named setting: persists it to the DB, applies any side effect (e.g. autostart toggle), and refreshes `SETTINGS_INST`.
pub async fn update_settings_value<R: Runtime>(
    app: &AppHandle<R>,
    name: &str,
    value: &str,
) -> Result<(), String> {
    let name = name.to_string();
    let value = value.to_string();
    let app_cl = app.clone();

    let lang = app.state::<SettingsLock>().read().unwrap().language;

    // Installed before persisting, so a cancelled password prompt leaves the setting untouched
    if name == ON_DEVICE_CONNECT
        && let Err(e) = install_udev_rules(app, value == "true").await
    {
        return Err(report_error(
            e,
            lang,
            "error_update_settings",
            "Error updating settings",
        ));
    }

    let res = tokio::task::spawn_blocking(move || {
        let app = app_cl;
        let database = app.state::<DatabasePool>();
        let settings = app.state::<SettingsLock>();
        match name.as_str() {
            DISTANCE_UNIT => {
                let value = value.as_str().try_into()?;
                crate::dao::settings::Settings::set_distance_unit(&database, &value)
                    .map_err(|e| e.to_string())?;
                settings.write().unwrap().distance_unit = value;
            }
            START_ON_BOOT => {
                let value = value == "true";
                crate::dao::settings::Settings::set_start_on_boot(&database, value)
                    .map_err(|e| e.to_string())?;
                if value {
                    app.autolaunch().enable()
                } else {
                    app.autolaunch().disable()
                }
                .map_err(|e| e.to_string())?;
                settings.write().unwrap().start_boot = value;
            }
            START_INTO_TRAY => {
                let value = value == "true";
                crate::dao::settings::Settings::set_start_into_tray(&database, value)
                    .map_err(|e| e.to_string())?;
                settings.write().unwrap().start_into_tray = value;
            }
            CLOSE_TO_TRAY => {
                let value = value == "true";
                crate::dao::settings::Settings::set_close_to_tray(&database, value)
                    .map_err(|e| e.to_string())?;
                settings.write().unwrap().close_to_tray = value;
            }
            WEIGHT_UNIT => {
                let value = value.as_str().try_into()?;
                crate::dao::settings::Settings::set_weight_unit(&database, &value)
                    .map_err(|e| e.to_string())?;
                settings.write().unwrap().weight_unit = value;
            }
            LANGUAGE => {
                let value = Languages::from_name(&value);
                crate::dao::settings::Settings::set_language(&database, &value)
                    .map_err(|e| e.to_string())?;
                settings.write().unwrap().language = value;
            }
            ON_DEVICE_CONNECT => {
                let value = value == "true";
                crate::dao::settings::Settings::set_on_device_connect(&database, value)
                    .map_err(|e| e.to_string())?;
                settings.write().unwrap().on_device_connect = value;
            }
            _ => unreachable!(),
        }

        Ok(settings.read().unwrap().language)
    })
    .await
    .map_err(|e| e.to_string())
    .flatten();

    match res {
        Ok(lang) => {
            show_notification(NotificationDefinition {
                title: translate("ok_update_settings", lang),
                body: String::new(),
                kind: NotificationKind::Temporal,
            });
            Ok(())
        }
        Err(e) => Err(report_error(
            e,
            lang,
            "error_update_settings",
            "Error updating settings",
        )),
    }
}

/// Returns every translation key resolved to the current UI language, for the frontend's i18n bootstrap.
pub async fn get_translations(settings: &SettingsLock) -> Result<HashMap<String, String>, String> {
    let lang = settings.read().unwrap().language;
    Ok(TRANSLATIONS
        .keys()
        .map(|k| (k.to_string(), translate(k, lang)))
        .collect())
}

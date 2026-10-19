use std::{collections::HashMap, thread, time::Duration};

use rusqlite_orm::database::DatabasePool;
use semver::Version;
use serde_json::Value;
use strength_tracker_rs_macros::traced_command;
use tauri::{AppHandle, Manager, State, WebviewWindowBuilder};
use tauri_plugin_autostart::ManagerExt;

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
        devices::start_device_watcher, notifications::show_notification, report_error,
        sessions::update_pending_geolocation,
    },
    udev::UdevManager,
    utils::translations::{Languages, TRANSLATIONS, translate, translate_and_replace},
};
use tauri_plugin_log::log::{debug, info};

/// Returns the current in-memory app settings.
#[traced_command]
#[tauri::command]
pub async fn get_settings(settings: State<'_, SettingsLock>) -> Result<Settings, String> {
    Ok(settings.read().unwrap().clone())
}

/// Called once the frontend has mounted: starts the USB device watcher and reveals the main window.
#[traced_command]
#[tauri::command]
pub async fn notify_frontend_ready(app: AppHandle) {
    info!("UI ready");

    start_device_watcher(app.clone());

    if !app.state::<SettingsLock>().read().unwrap().start_into_tray {
        let _ = show_main_window(&app);
    }

    let app_cl = app.clone();
    std::thread::spawn(move || {
        let db = app_cl.state::<DatabasePool>();
        update_pending_geolocation(&app_cl, &db);
    });

    let app_cl = app.clone();
    std::thread::spawn(|| check_for_update(app_cl));
}

pub fn show_main_window(app: &AppHandle) -> tauri::Result<()> {
    if let Some(w) = app.get_webview_window("main") {
        w.show()?;
        w.unminimize()?;
        w.set_focus()?;

        return Ok(());
    }

    let config = app
        .config()
        .app
        .windows
        .iter()
        .find(|w| w.label == "main")
        .expect("no hay ventana 'main' en tauri.conf.json")
        .clone();

    let w = WebviewWindowBuilder::from_config(app, &config)?.build()?;
    w.set_focus()?;

    Ok(())
}

pub fn hide_main_window(app: &AppHandle) -> tauri::Result<()> {
    if let Some(w) = app.get_webview_window("main") {
        w.hide()?;
        return Ok(());
    }
    Ok(())
}

/// Reports whether this is a debug or release build.
#[traced_command]
#[tauri::command]
pub async fn get_environment() -> AppEnvironment {
    if cfg!(debug_assertions) {
        AppEnvironment::Debug
    } else {
        AppEnvironment::Release
    }
}

/// Updates a single named setting: persists it to the DB, applies any side effect (e.g. autostart toggle), and refreshes `SETTINGS_INST`.
#[traced_command]
#[tauri::command]
pub async fn update_settings_value(app: AppHandle, name: &str, value: &str) -> Result<(), String> {
    let name = name.to_string();
    let value = value.to_string();

    let lang = app.state::<SettingsLock>().read().unwrap().language;

    let res = tokio::task::spawn_blocking(move || {
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

                info!("Installing udev rules...");
                UdevManager::write_rules_file(value).map_err(|e| e.to_string())?;

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
#[traced_command(log_payload = false)]
#[tauri::command]
pub async fn get_translations(
    settings: State<'_, SettingsLock>,
) -> Result<HashMap<String, String>, String> {
    let lang = settings.read().unwrap().language;
    Ok(TRANSLATIONS
        .keys()
        .map(|k| (k.to_string(), translate(k, lang)))
        .collect())
}

fn check_for_update(app: AppHandle) {
    thread::sleep(Duration::from_secs(5));

    loop {
        info!("Looking for updates...");

        let result = curl_rest::Client::with_user_agent("strength-tracker-rs")
            .get()
            .header(curl_rest::Header::Accept(
                "application/vnd.github+json".into(),
            ))
            .send(constants::URL);

        match result {
            Ok(response) => {
                if response.status == curl_rest::StatusCode::Ok {
                    match serde_json::from_slice::<Value>(&response.body) {
                        Ok(response_json) => match response_json.get("tag_name") {
                            Some(tag_name) => match Version::parse(tag_name.as_str().unwrap()) {
                                Ok(version) => {
                                    if version > *constants::APP_SEM_VERSION {
                                        info!("Update {} found!", version);
                                        let lang =
                                            app.state::<SettingsLock>().read().unwrap().language;
                                        show_notification(NotificationDefinition {
                                            title: translate("new_update_title", lang),
                                            body: translate_and_replace(
                                                "new_update_body",
                                                &[&version.to_string()],
                                                lang,
                                            ),
                                            kind: NotificationKind::Persistant,
                                        });
                                        break;
                                    } else {
                                        info!("No update found");
                                    }
                                }
                                Err(e) => {
                                    debug!("Error parsing version: {}", e)
                                }
                            },
                            None => {
                                debug!("No tag name found")
                            }
                        },
                        Err(e) => {
                            debug!("Invalid parsing response: {}", e)
                        }
                    }
                } else {
                    debug!("Invalid response status: {}", response.status,)
                }
            }
            Err(e) => {
                debug!("Error sending request to GitHub: {}", e)
            }
        }
        thread::sleep(Duration::from_hours(1));
    }
}

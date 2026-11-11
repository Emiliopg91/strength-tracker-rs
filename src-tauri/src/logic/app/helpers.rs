use std::time::Duration;

use semver::Version;
use serde_json::Value;
use tauri::{AppHandle, Manager, Runtime, WebviewWindowBuilder};
use tauri_plugin_log::log::{debug, info, warn};

use crate::{
    SettingsLock, constants,
    dto::{
        notifications::{NotificationDefinition, NotificationKind},
        prompts::PromptKind,
    },
    logic::{notifications::show_notification, prompts::helpers::request_prompt},
    udev::{UdevManager, errors::UdevError},
    utils::translations::{translate, translate_and_replace},
};

pub(crate) fn show_main_window<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<()> {
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
        .expect("no 'main' window in tauri.conf.json")
        .clone();

    let w = WebviewWindowBuilder::from_config(app, &config)?.build()?;
    w.set_focus()?;

    Ok(())
}

pub(crate) fn hide_main_window<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<()> {
    if let Some(w) = app.get_webview_window("main") {
        w.hide()?;
        return Ok(());
    }
    Ok(())
}

/// Writes the udev rules, asking for the sudo password through the frontend unless already root.
/// A rejected password is asked again; fails if the user cancels.
pub(super) async fn install_udev_rules<R: Runtime>(
    app: &AppHandle<R>,
    auto_run: bool,
) -> Result<(), String> {
    info!("Installing udev rules...");
    if UdevManager::is_root() {
        return tokio::task::spawn_blocking(move || UdevManager::write_rules_file(auto_run))
            .await
            .map_err(|e| e.to_string())?
            .map_err(|e| e.to_string());
    }

    let mut detail = None;
    loop {
        let Some(password) = request_prompt(app, PromptKind::SudoPassword, None, detail.take())
            .await
            .and_then(|values| values.into_iter().next())
        else {
            return Err("Udev rules installation cancelled by the user".to_string());
        };

        let res = tokio::task::spawn_blocking(move || {
            UdevManager::write_rules_file_with_sudo(auto_run, &password)
        })
        .await
        .map_err(|e| e.to_string())?;

        match res {
            Ok(()) => {
                info!("Udev rules installed");
                return Ok(());
            }
            Err(e @ UdevError::WrongPassword) => {
                warn!("Udev rules installation failed: {e}");
                detail = Some(e.to_string());
            }
            Err(e) => return Err(e.to_string()),
        }
    }
}

pub(super) async fn check_for_update<R: Runtime>(app: &AppHandle<R>) {
    tokio::time::sleep(Duration::from_secs(5)).await;

    loop {
        info!("Looking for updates...");
        match latest_version().await {
            Ok(version) if version > *constants::APP_SEM_VERSION => {
                info!("Update {} found!", version);
                let lang = app.state::<SettingsLock>().read().unwrap().language;
                show_notification(NotificationDefinition {
                    title: translate("new_update_title", lang),
                    body: translate_and_replace("new_update_body", &[&version.to_string()], lang),
                    kind: NotificationKind::Persistant,
                });
                break;
            }
            Ok(_) => info!("No update found"),
            Err(e) => debug!("{e}"),
        }
        tokio::time::sleep(Duration::from_hours(1)).await;
    }
}

/// Latest version published on GitHub.
async fn latest_version() -> Result<Version, String> {
    let response = tokio::task::spawn_blocking(|| {
        curl_rest::Client::with_user_agent("strength-tracker-rs")
            .get()
            .header(curl_rest::Header::Accept(
                "application/vnd.github+json".into(),
            ))
            .send(constants::URL)
    })
    .await
    .expect("Update check task panicked")
    .map_err(|e| format!("Error sending request to GitHub: {e}"))?;
    if response.status != curl_rest::StatusCode::Ok {
        return Err(format!("Invalid response status: {}", response.status));
    }

    let json: Value = serde_json::from_slice(&response.body)
        .map_err(|e| format!("Invalid parsing response: {e}"))?;
    let tag_name = json
        .get("tag_name")
        .and_then(Value::as_str)
        .ok_or("No tag name found")?;
    Version::parse(tag_name).map_err(|e| format!("Error parsing version: {e}"))
}

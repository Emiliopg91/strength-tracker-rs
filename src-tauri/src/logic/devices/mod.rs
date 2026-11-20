pub mod commands;
pub mod helpers;

use chrono::{Datelike, Local, TimeZone, Timelike};
use rusqlite_orm::errors::DatabaseError;
use tauri::{AppHandle, Manager, Runtime};
use tauri_plugin_log::log::{error, info};

use self::helpers::{
    notify_workout_pending, notify_workout_sent, register_workout_on_device,
    send_workout_to_device, set_workout_on_device, workout_name,
};
use crate::{
    MtpLock, SettingsLock,
    dao::device::Device,
    dto::devices::DeviceListItem,
    logic::{
        fit_settings, language, pool, report_error,
        sessions::helpers::{finish_session_import, import_pending_sessions},
    },
    utils::constants,
};

pub async fn send_to_device<R: Runtime>(
    app: &AppHandle<R>,
    settings: &SettingsLock,
    id: u16,
    serial: &str,
) -> Result<(), String> {
    let (lang, weight_unit) = fit_settings(app);
    let workout = &workout_name(app, id).await;

    let mtp_client = app.state::<MtpLock>();
    let connected = mtp_client
        .lock()
        .await
        .is_device_connected(serial)
        .await
        .map_err(|e| e.to_string())?;

    let res: Result<bool, String> = if connected {
        match send_workout_to_device(app, lang, weight_unit, id, serial).await {
            Ok(_) => {
                // Register the pair so later changes (e.g. progressions) get re-sent to this device
                if let Err(e) = register_workout_on_device(app, id, serial).await {
                    error!("Error registering workout '{workout}' for device {serial}: {e}");
                }

                Ok(true)
            }
            Err(e) => Err(e),
        }
    } else {
        set_workout_on_device(app, id, serial, true)
            .await
            .map_err(|e| e.to_string())?;

        Ok(false)
    };

    match res {
        Ok(true) => {
            notify_workout_sent(workout, serial, lang);
            Ok(())
        }
        Ok(false) => {
            notify_workout_pending(workout, serial, lang);
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

/// Downloads new activity files from the given device since its last sync and imports them.
pub async fn import_from_device<R: Runtime>(
    app: &AppHandle<R>,
    serial: &str,
) -> Result<usize, String> {
    info!("Starting import from device with S/N {}", serial);
    let mut latest_date = "1970-01-01-00-00-00".to_string();
    let lang = language(app);
    let serial_str = serial.to_string();
    let mut device = pool(app)
        .run_in_connection(async move |conn| Ok(Device::get(conn, &serial_str).await?))
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("Device {serial} is not registered"))?;

    if let Some(latest) = device.last_activity_sync {
        let latest = Local.timestamp_opt(latest as i64, 0).unwrap();
        latest_date = format!(
            "{:04}-{:02}-{:02}-{:02}-{:02}-{:02}",
            latest.year(),
            latest.month(),
            latest.day(),
            latest.hour(),
            latest.minute(),
            latest.second(),
        );
    }

    info!(
        "Fetching from device activity files after {}...",
        latest_date
    );
    let mut res: Result<Vec<u32>, DatabaseError> = Ok(Vec::new());
    let mtp_client = app.state::<MtpLock>();
    let mtp_client = mtp_client.lock().await;

    if let Ok(files) = mtp_client
        .download_activities_since(serial, latest_date)
        .await
    {
        drop(mtp_client);
        let now = Local::now().timestamp() as u32;
        res = import_pending_sessions(app, files, &mut device, lang, Some(now)).await;
    }

    finish_session_import(app, res)
}

pub async fn get_registered_devices<R: Runtime>(
    app: &AppHandle<R>,
) -> Result<Vec<DeviceListItem>, String> {
    Ok(pool(app)
        .run_in_connection(async move |conn| Ok(Device::get_all(conn).await?))
        .await
        .map_err(|e| e.to_string())?
        .iter()
        .filter_map(|d| {
            if d.serial != constants::CONNECT_DEVICE_SERIAL {
                Some(DeviceListItem::from(d))
            } else {
                None
            }
        })
        .collect())
}

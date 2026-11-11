pub mod commands;
pub mod helpers;
pub mod queries;

use chrono::{Datelike, Local, TimeZone, Timelike};
use rusqlite_orm::errors::DatabaseError;
use tauri::{AppHandle, Manager, Runtime};
use tauri_plugin_log::log::{error, info};

use self::helpers::send_workout_to_device;
use self::queries::{add_workout_device, all_devices, device_by_serial, queue_workout_device};
use crate::{
    MtpLock, SettingsLock,
    dao::workout_device::WorkoutDevice,
    dto::{
        devices::DeviceListItem,
        notifications::{NotificationDefinition, NotificationKind},
    },
    logic::{
        notifications::show_notification,
        pool, report_error,
        sessions::helpers::{import_pending_sessions, spawn_geolocation_update},
    },
    utils::{constants, translations::translate},
};

pub async fn send_to_device<R: Runtime>(
    app: &AppHandle<R>,
    settings: &SettingsLock,
    workout: &str,
    serial: &str,
) -> Result<(), String> {
    let (lang, weight_unit) = {
        let settings = settings.read().unwrap();
        (settings.language, settings.weight_unit)
    };

    let mtp_client = app.state::<MtpLock>();
    let connected = mtp_client
        .lock()
        .await
        .is_device_connected(serial)
        .await
        .map_err(|e| e.to_string())?;

    let res: Result<bool, String> = if connected {
        match send_workout_to_device(app, lang, weight_unit, workout, serial).await {
            Ok(_) => {
                // Register the pair so later changes (e.g. progressions) get re-sent to this device
                let mut entry = WorkoutDevice {
                    device: serial.to_string(),
                    workout: workout.to_string(),
                    pending: false,
                };

                if let Err(e) = add_workout_device(&pool(app), &mut entry).await {
                    error!("Error registering workout '{workout}' for device {serial}: {e}");
                }

                Ok(true)
            }
            Err(e) => Err(e),
        }
    } else {
        let mut entry = WorkoutDevice {
            device: serial.to_string(),
            workout: workout.to_string(),
            pending: true,
        };

        queue_workout_device(&pool(app), &mut entry)
            .await
            .map_err(|e| e.to_string())?;

        Ok(false)
    };

    match res {
        Ok(true) => {
            info!("Workout '{}' sent to device {}", workout, serial);
            show_notification(NotificationDefinition {
                title: workout.to_string(),
                body: translate("ok_workout_send", lang),
                kind: NotificationKind::Temporal,
            });
            Ok(())
        }
        Ok(false) => {
            info!(
                "Workout '{}' pending to be sent to device {}",
                workout, serial
            );
            show_notification(NotificationDefinition {
                title: workout.to_string(),
                body: translate("pending_workout_send", lang),
                kind: NotificationKind::Temporal,
            });
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
    let lang = app.state::<SettingsLock>().read().unwrap().language;
    let mut device = device_by_serial(&pool(app), serial)
        .await
        .map_err(|e| e.to_string())?;

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

    match res {
        Ok(res) => {
            if !res.is_empty() {
                spawn_geolocation_update(app);
            }
            Ok(res.len())
        }
        Err(e) => Err(report_error(
            e,
            &app.state::<SettingsLock>(),
            "error_import_sessions",
            "Error importing sessions",
        )),
    }
}

pub async fn get_registered_devices<R: Runtime>(
    app: &AppHandle<R>,
) -> Result<Vec<DeviceListItem>, String> {
    Ok(all_devices(&pool(app))
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

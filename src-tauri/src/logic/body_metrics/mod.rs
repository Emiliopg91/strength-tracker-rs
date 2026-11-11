pub mod commands;
pub mod queries;

use tauri::{AppHandle, Runtime};
use tauri_plugin_log::log::info;

use self::queries::{delete_body_metric_on, fetch_body_measures, insert_body_metric};
use crate::{
    SettingsLock,
    dto::{
        body_metrics::BodyMetricListItem,
        notifications::{NotificationDefinition, NotificationKind},
    },
    logic::{notifications::show_notification, pool, report_error},
    utils::translations::translate,
};

/// Returns all logged body measurements, newest first.
pub async fn get_body_measures<R: Runtime>(
    app: &AppHandle<R>,
    settings: &SettingsLock,
) -> Result<Vec<BodyMetricListItem>, String> {
    info!("Getting body measures list...");

    pool(app)
        .run_in_connection(move |conn| Ok(fetch_body_measures(conn, None)?))
        .await
        .inspect(|res| info!("Retrieved {} measures", res.len()))
        .map_err(|e| {
            report_error(
                e,
                settings,
                "error_body_measures_list",
                "Error getting measures list",
            )
        })
}

/// Inserts a new body measurement entry.
pub async fn add_body_measures<R: Runtime>(
    app: &AppHandle<R>,
    settings: &SettingsLock,
    measures: BodyMetricListItem,
) -> Result<(), String> {
    info!("Adding body measures list...");

    let res = insert_body_metric(&pool(app), &measures).await;

    match res {
        Ok(_) => {
            info!("Measures added succesfully");
            Ok(())
        }
        Err(e) => Err(report_error(
            e,
            settings,
            "error_adding_body_measures",
            "Error adding measures",
        )),
    }
}

/// Deletes the body measurement logged on `date`, if any.
pub async fn delete_body_metric<R: Runtime>(
    app: &AppHandle<R>,
    settings: &SettingsLock,
    date: u32,
) -> Result<(), String> {
    let res = delete_body_metric_on(&pool(app), date).await;

    let lang = settings.read().unwrap().language;
    match res {
        Ok(_) => {
            info!("Measures deleted succesfully");
            show_notification(NotificationDefinition {
                title: translate("ok_delete_body_entry", lang),
                body: "".to_string(),
                kind: NotificationKind::Temporal,
            });
            Ok(())
        }
        Err(e) => Err(report_error(
            e,
            settings,
            "error_deleting_body_measures",
            "Error deleting measures",
        )),
    }
}

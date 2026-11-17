pub mod commands;

use tauri::{AppHandle, Runtime};
use tauri_plugin_log::log::info;

use crate::{
    SettingsLock,
    dao::body_metric::BodyMetric,
    dto::{
        body_metrics::BodyMetricListItem,
        notifications::{NotificationDefinition, NotificationKind},
    },
    logic::{notifications::show_notification, pool, report_error, settings_language},
    utils::translations::translate,
};

/// Returns all logged body measurements, newest first.
pub async fn get_body_measures<R: Runtime>(
    app: &AppHandle<R>,
    settings: &SettingsLock,
) -> Result<Vec<BodyMetricListItem>, String> {
    info!("Getting body measures list...");

    pool(app)
        .run_in_connection(async move |conn| {
            Ok(BodyMetric::get_by_range(conn, None)
                .await?
                .iter()
                .map(BodyMetricListItem::from)
                .collect::<Vec<_>>())
        })
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

    let res = BodyMetric::insert(
        &pool(app),
        &mut BodyMetric::try_from(&measures).map_err(|e| e.to_string())?,
    )
    .await;

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
    let res = BodyMetric::delete_on(&pool(app), date).await;

    let lang = settings_language(settings);
    match res {
        Ok(_) => {
            info!("Measures deleted succesfully");
            show_notification(NotificationDefinition {
                title: translate("ok_delete_body_entry", lang),
                body: String::new(),
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

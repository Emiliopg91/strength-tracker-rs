use rusqlite_orm::{
    dao::Repository,
    errors::DatabaseError,
    types::{order_by::OrderBy, where_clause::Where},
};
use strength_tracker_rs_macros::traced_command;
use tauri::{AppHandle, State};
use tauri_plugin_log::log::info;

use crate::{
    SettingsLock,
    dao::body_metric::{self, BodyMetric, BodyMetricRepository},
    dto::{
        body_metrics::BodyMetricListItem,
        notifications::{NotificationDefinition, NotificationKind},
    },
    logic::{notifications::show_notification, report_error, run_blocking},
    utils::translations::translate,
};

/// Returns all logged body measurements, newest first.
#[traced_command]
#[tauri::command]
pub async fn get_body_measures(
    app: AppHandle,
    settings: State<'_, SettingsLock>,
) -> Result<Vec<BodyMetricListItem>, String> {
    info!("Getting body measures list...");

    let res = run_blocking(app, move |database| {
        database.run_in_connection(|conn| {
            let regs = BodyMetricRepository::select()
                .order_by(OrderBy::Desc(body_metric::entity::columns::DATE))
                .fetch_in(conn)?;

            Ok(regs)
        })
    })
    .await;

    match res {
        Ok(regs) => {
            let res = regs
                .iter()
                .map(BodyMetricListItem::from)
                .collect::<Vec<BodyMetricListItem>>();

            info!("Retrieved {} measures", res.len());
            Ok(res)
        }
        Err(e) => Err(report_error(
            e,
            settings.read().unwrap().language,
            "error_body_measures_list",
            "Error getting measures list",
        )),
    }
}

/// Inserts a new body measurement entry.
#[traced_command]
#[tauri::command]
pub async fn add_body_measures(
    app: AppHandle,
    settings: State<'_, SettingsLock>,
    measures: BodyMetricListItem,
) -> Result<(), String> {
    info!("Adding body measures list...");

    let res = run_blocking(app, move |database| {
        database.run_in_transaction(|tx| {
            BodyMetricRepository::insert()
                .item(&mut BodyMetric::try_from(&measures).map_err(DatabaseError::Transaction)?)
                .execute_in(tx)?;

            Ok(())
        })
    })
    .await;

    match res {
        Ok(_) => {
            info!("Measures added succesfully");
            Ok(())
        }
        Err(e) => Err(report_error(
            e,
            settings.read().unwrap().language,
            "error_adding_body_measures",
            "Error adding measures",
        )),
    }
}

/// Deletes the body measurement logged on `date`, if any.
#[traced_command]
#[tauri::command]
pub async fn delete_body_metric(
    app: AppHandle,
    settings: State<'_, SettingsLock>,
    date: u32,
) -> Result<(), String> {
    let res = run_blocking(app, move |database| {
        database.run_in_transaction(|tx| {
            BodyMetricRepository::delete()
                .where_(Where::Eq(body_metric::entity::columns::DATE, date.into()))
                .execute_in(tx)?;

            Ok(())
        })
    })
    .await;

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
            lang,
            "error_deleting_body_measures",
            "Error deleting measures",
        )),
    }
}

pub mod app;
pub mod body_metrics;
pub mod devices;
pub mod exercises;
pub mod export;
pub mod notifications;
pub mod sessions;
pub mod workouts;

use rusqlite_orm::{
    database::DatabasePool,
    types::{row::Row, value::Value},
};
use tauri::{AppHandle, Manager};
use tauri_plugin_log::log::error;

use crate::{
    dto::notifications::{NotificationDefinition, NotificationKind},
    logic::notifications::show_notification,
    utils::translations::{Languages, translate},
};

/// Logs `e` (prefixed by `log_msg`), fires a persistent desktop notification titled by the
/// translation key `error_key`, and returns the stringified error for the command's
/// `Result<T, String>`.
pub fn report_error<E: std::fmt::Display>(
    e: E,
    lang: Languages,
    error_key: &str,
    log_msg: &str,
) -> String {
    error!("{}: {}", log_msg, e);
    show_notification(NotificationDefinition {
        title: translate(error_key, lang),
        body: e.to_string(),
        kind: NotificationKind::Persistant,
    });
    e.to_string()
}

/// Runs `f` with the database pool on tokio's blocking thread pool, so synchronous DB and file
/// work never stalls an async worker thread.
pub async fn run_blocking<T, F>(app: AppHandle, f: F) -> T
where
    F: FnOnce(&DatabasePool) -> T + Send + 'static,
    T: Send + 'static,
{
    tokio::task::spawn_blocking(move || f(&app.state::<DatabasePool>()))
        .await
        .expect("blocking DB task panicked")
}

/// Reads an integer column from a non-mapped select row.
pub(crate) fn row_int(row: &Row, column: &str) -> i64 {
    match row.get(column) {
        Some(Value::Int64(v)) => *v,
        _ => unreachable!(),
    }
}

/// Reads a real column from a non-mapped select row.
pub(crate) fn row_f64(row: &Row, column: &str) -> f64 {
    match row.get(column) {
        Some(Value::Float64(v)) => *v,
        Some(Value::Int64(v)) => *v as f64,
        _ => unreachable!(),
    }
}

/// Reads a text column from a non-mapped select row.
pub(crate) fn row_text(row: &Row, column: &str) -> String {
    match row.get(column) {
        Some(Value::Text(v)) => v.clone(),
        _ => unreachable!(),
    }
}

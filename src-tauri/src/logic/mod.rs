pub mod app;
pub mod body_metrics;
pub mod connect;
pub mod devices;
pub mod exercises;
pub mod export;
pub mod notifications;
pub mod report;
pub mod sessions;
pub mod workouts;

use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

use chrono::Local;

use rusqlite_orm::{
    database::DatabasePool,
    types::{row::Row, value::Value},
};
use tauri::{AppHandle, Manager};
use tauri_plugin_log::log::error;

use crate::{
    dto::notifications::{NotificationDefinition, NotificationKind},
    logic::notifications::show_notification,
    utils::{
        constants,
        translations::{Languages, translate},
    },
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
pub async fn run_blocking<T, F>(app: &AppHandle, f: F) -> T
where
    F: FnOnce(&DatabasePool) -> T + Send + 'static,
    T: Send + 'static,
{
    let app = app.clone();
    tokio::task::spawn_blocking(move || f(&app.state::<DatabasePool>()))
        .await
        .expect("blocking DB task panicked")
}

/// Lists the `.fit` files directly inside `dir`, which leaves out those already marked as failed.
pub(crate) fn list_fit_files(dir: &Path) -> Vec<PathBuf> {
    fs::read_dir(dir)
        .map(|read_dir| {
            read_dir
                .flatten()
                .map(|entry| entry.path())
                .filter(|path| {
                    path.extension()
                        .is_some_and(|ext| ext.eq_ignore_ascii_case("fit"))
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Gives every transfer its own dir, so concurrent syncs of the same source don't share files.
static PENDING_DIR_SEQ: AtomicU64 = AtomicU64::new(0);

/// Returns a new dir under `root` for a single transfer of `serial`. The timestamp keeps it apart
/// from those left by previous runs, and the sequence from concurrent transfers.
pub(crate) fn new_pending_dir(root: &Path, serial: &str) -> PathBuf {
    root.join(serial).join(format!(
        "{}-{}",
        Local::now().timestamp_millis(),
        PENDING_DIR_SEQ.fetch_add(1, Ordering::Relaxed)
    ))
}

/// Settles pending downloads once their import is over: `done` files are removed and `failed` ones
/// get the `.failed` suffix, so they are kept for debugging but not picked up again. `dir` is
/// removed once left empty.
pub(crate) fn settle_pending_files(dir: &Path, done: &[PathBuf], failed: &[PathBuf]) {
    for file in done {
        let _ = fs::remove_file(file);
    }
    for file in failed {
        let mut dst = file.clone().into_os_string();
        dst.push(constants::FAILED_FILE_SUFFIX);
        if let Err(e) = fs::rename(file, &dst) {
            error!("Error marking {} as failed: {e}", file.display());
        }
    }
    // Fails while anything is left in it
    let _ = fs::remove_dir(dir);
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

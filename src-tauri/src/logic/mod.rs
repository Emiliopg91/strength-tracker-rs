pub mod app;
pub mod body_metrics;
pub mod connect;
pub mod devices;
pub mod exercises;
pub mod export;
pub mod notifications;
pub mod prompts;
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
    errors::DatabaseError,
    rusqlite::{Connection, Transaction},
};
use tauri::{AppHandle, Manager, Runtime};
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_log::log::{error, warn};
use tokio::sync::oneshot;

use crate::{
    dto::notifications::{NotificationDefinition, NotificationKind},
    fit::FitFile,
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
pub async fn run_blocking<R, T, F>(app: &AppHandle<R>, f: F) -> T
where
    R: Runtime,
    F: FnOnce(&DatabasePool) -> T + Send + 'static,
    T: Send + 'static,
{
    let app = app.clone();
    tokio::task::spawn_blocking(move || f(&app.state::<DatabasePool>()))
        .await
        .expect("blocking DB task panicked")
}

/// Runs `f` on a pooled connection in tokio's blocking thread pool.
pub async fn run_blocking_conn<R, T, F>(app: &AppHandle<R>, f: F) -> Result<T, DatabaseError>
where
    R: Runtime,
    F: FnOnce(&AppHandle<R>, &Connection) -> Result<T, DatabaseError> + Send + 'static,
    T: Send + 'static,
{
    let app = app.clone();
    tokio::task::spawn_blocking(move || {
        let mut f = Some(f);
        app.state::<DatabasePool>()
            .run_in_connection(|conn| Ok(f.take().unwrap()(&app, conn)?))
    })
    .await
    .expect("blocking DB task panicked")
}

/// Runs `f` in a transaction in tokio's blocking thread pool, committing it if `f` succeeds.
pub async fn run_blocking_tx<R, T, F>(app: &AppHandle<R>, f: F) -> Result<T, DatabaseError>
where
    R: Runtime,
    F: FnOnce(&AppHandle<R>, &mut Transaction) -> Result<T, DatabaseError> + Send + 'static,
    T: Send + 'static,
{
    let app = app.clone();
    tokio::task::spawn_blocking(move || {
        let mut f = Some(f);
        app.state::<DatabasePool>()
            .run_in_transaction(|tx| Ok(f.take().unwrap()(&app, tx)?))
    })
    .await
    .expect("blocking DB task panicked")
}

/// Adds `extension` to `path` unless it already ends in it.
pub(crate) fn ensure_extension(mut path: PathBuf, extension: &str) -> PathBuf {
    if path
        .extension()
        .is_none_or(|ext| !ext.eq_ignore_ascii_case(extension))
    {
        path.as_mut_os_string().push(format!(".{extension}"));
    }
    path
}

/// Asks where to save a file, starting at `default_file` and offering only files with
/// `extension`, which is appended if missing. Returns `None` if the user cancels.
pub(crate) async fn ask_save_path<R: Runtime>(
    app: &AppHandle<R>,
    default_file: &Path,
    filter_name: &str,
    extension: &str,
    title: &str,
) -> Result<Option<PathBuf>, String> {
    let mut dialog = app
        .dialog()
        .file()
        .set_title(title)
        .add_filter(filter_name, &[extension]);
    if let Some(dir) = default_file.parent() {
        dialog = dialog.set_directory(dir);
    }
    if let Some(name) = default_file.file_name() {
        dialog = dialog.set_file_name(name.to_string_lossy());
    }

    let (tx, rx) = oneshot::channel();
    dialog.save_file(move |path| {
        let _ = tx.send(path);
    });

    match rx.await.map_err(|e| e.to_string())? {
        Some(path) => Ok(Some(ensure_extension(
            path.into_path().map_err(|e| e.to_string())?,
            extension,
        ))),
        None => Ok(None),
    }
}

/// Asks for one or more existing files with `extension`. Empty if the user cancels.
pub(crate) async fn ask_open_paths<R: Runtime>(
    app: &AppHandle<R>,
    filter_name: &str,
    extension: &str,
    title: &str,
) -> Result<Vec<PathBuf>, String> {
    let (tx, rx) = oneshot::channel();
    app.dialog()
        .file()
        .set_title(title)
        .add_filter(filter_name, &[extension])
        .pick_files(move |paths| {
            let _ = tx.send(paths);
        });

    rx.await
        .map_err(|e| e.to_string())?
        .unwrap_or_default()
        .into_iter()
        .map(|path| path.into_path().map_err(|e| e.to_string()))
        .collect()
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

/// Stores under `root/<serial>` the fetched `files` whose parse or import failed, so they are kept
/// for a later look instead of being lost. A file with the same name is replaced.
pub(crate) fn store_pending_files<'a, I>(root: &Path, serial: &str, files: I)
where
    I: IntoIterator<Item = &'a FitFile>,
{
    let dir = root.join(serial);
    for file in files {
        // Names come from the device or a Connect archive, so only the last component is kept
        let Some(name) = Path::new(&file.name).file_name() else {
            error!("Invalid name for pending file '{}'", file.name);
            continue;
        };
        let dst = dir.join(name);
        let res = fs::create_dir_all(&dir).and_then(|_| fs::write(&dst, &file.data));
        match res {
            Ok(()) => warn!("Stored pending file {}", dst.display()),
            Err(e) => error!("Error storing pending file {}: {e}", dst.display()),
        }
    }
}

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
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
};

use chrono::Local;
use rusqlite_orm::database::DatabasePool;
use tauri::{AppHandle, Manager, Runtime, WebviewWindow};
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_log::log::{error, info, warn};
use tokio::sync::oneshot;

use crate::{
    SettingsLock,
    dto::notifications::{NotificationDefinition, NotificationKind},
    fit::FitFile,
    logic::notifications::show_notification,
    utils::translations::translate,
};

/// Logs `e` (prefixed by `log_msg`), fires a persistent desktop notification titled by the
/// translation key `error_key`, and returns the stringified error for the command's
/// `Result<T, String>`.
pub fn report_error<E: std::fmt::Display>(
    e: E,
    settings: &SettingsLock,
    error_key: &str,
    log_msg: &str,
) -> String {
    error!("{}: {}", log_msg, e);
    show_notification(NotificationDefinition {
        title: translate(error_key, settings.read().unwrap().language),
        body: e.to_string(),
        kind: NotificationKind::Persistant,
    });
    e.to_string()
}

/// The app's database pool. Clones are cheap and share the same connections.
pub fn pool<R: Runtime>(app: &AppHandle<R>) -> DatabasePool {
    app.state::<DatabasePool>().inner().clone()
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

/// Asks where to save `w` as a PDF, starting at `<downloads>/<default_name>.pdf`, prints it there
/// and opens it. The window is closed once printing ends, on cancel or on error.
pub(crate) async fn print_to_pdf<R: Runtime>(
    w: WebviewWindow<R>,
    default_name: &str,
) -> Result<(), String> {
    let res = start_pdf_print(w.clone(), default_name).await;
    // On success the window is closed once the print operation ends
    if res.is_err() {
        let _ = w.close();
    }
    res
}

async fn start_pdf_print<R: Runtime>(
    w: WebviewWindow<R>,
    default_name: &str,
) -> Result<(), String> {
    use webkit2gtk::{PrintOperation, PrintOperationExt};

    let lang = w.state::<SettingsLock>().read().unwrap().language;
    let default_file = w
        .path()
        .download_dir()
        .map_err(|e| e.to_string())?
        .join(format!("{default_name}.pdf"));

    let Some(file) = ask_save_path(
        w.app_handle(),
        &default_file,
        "PDF",
        "pdf",
        &translate("select_file_save_pdf", lang),
    )
    .await?
    else {
        // Cancelled by the user
        let _ = w.close();
        return Ok(());
    };

    let uri = format!("file://{}", file.display());
    w.clone()
        .with_webview(move |pw| {
            let settings = gtk::PrintSettings::new();
            // GTK registers its file backend under a localized name
            settings.set_printer(&gtk::glib::dgettext(Some("gtk30"), "Print to File"));
            settings.set(gtk::PRINT_SETTINGS_OUTPUT_FILE_FORMAT, Some("pdf"));
            settings.set(gtk::PRINT_SETTINGS_OUTPUT_URI, Some(&uri));

            let page_setup = gtk::PageSetup::new();
            page_setup.set_paper_size(&gtk::PaperSize::new(Some(gtk::PAPER_NAME_A4)));
            page_setup.set_top_margin(15.0, gtk::Unit::Mm);
            page_setup.set_bottom_margin(15.0, gtk::Unit::Mm);
            page_setup.set_left_margin(15.0, gtk::Unit::Mm);
            page_setup.set_right_margin(15.0, gtk::Unit::Mm);

            let op = PrintOperation::new(&pw.inner());
            op.set_print_settings(&settings);
            op.set_page_setup(&page_setup);
            op.connect_failed(move |_, e| {
                error!("PDF print failed: {e}");
            });
            // Emitted after `failed` too, so the hidden window is always released here
            op.connect_finished(move |_| {
                info!("PDF printed to {uri}");
                let _ = Command::new("xdg-open").arg(&uri).status();
                let _ = w.close();
            });
            op.print();
        })
        .map_err(|e| e.to_string())
}

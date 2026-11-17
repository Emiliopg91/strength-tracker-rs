pub mod commands;
pub mod helpers;

use tauri::{AppHandle, Runtime, WebviewWindow};
use tauri_plugin_log::log::info;

use self::helpers::report_data;
use crate::{
    SettingsLock,
    dto::report::ReportData,
    logic::{language, open_window, pool, print_to_pdf, report_error},
    utils::translations::translate,
};

/// Opens the report window for the `[from, to]` range, which stays hidden: printing starts once
/// the page calls `report_ready`.
pub async fn load_report<R: Runtime>(app: &AppHandle<R>, from: u32, to: u32) -> Result<(), String> {
    open_window(app, "report", Some(&format!("from={from}&to={to}")))
        .map(|_| ())
        .map_err(|e| e.to_string())
}

/// Called by the report window once everything is rendered and painted: prints it to PDF.
pub async fn report_ready(window: WebviewWindow) -> Result<(), String> {
    info!("Report ready, printing");
    let lang = language(&window);
    print_to_pdf(window, &translate("progression_report", lang)).await
}

/// Gathers everything the report needs for the `[from, to]` range, reusing the commands' DB logic on one connection.
pub async fn get_report_data<R: Runtime>(
    app: &AppHandle<R>,
    settings: &SettingsLock,
    from: u32,
    to: u32,
) -> Result<ReportData, String> {
    info!("Getting report data...");
    let lang = settings.read().unwrap().language;

    let res = pool(app)
        .run_in_connection(async move |conn| Ok(report_data(conn, from, to).await?))
        .await
        .map(|mut data| {
            // Listed by name, which depends on the language
            data.exercises
                .sort_by_key(|e| translate(&format!("exercise_{}_{}", e.category, e.id), lang));
            data
        });

    res.inspect(|data| {
        info!(
            "Retrieved report data: {} sessions, {} exercises, {} body metrics",
            data.sessions.len(),
            data.exercises.len(),
            data.body_metrics.len()
        )
    })
    .map_err(|e| {
        report_error(
            e,
            settings,
            "error_report_data",
            "Error getting report data",
        )
    })
}

pub mod commands;
pub mod queries;

use tauri::{AppHandle, Manager, Runtime, WebviewWindow, WebviewWindowBuilder};
use tauri_plugin_log::log::info;

use self::queries::report_data;
use crate::{
    SettingsLock,
    dto::report::ReportData,
    logic::{pool, print_to_pdf, report_error},
    utils::translations::translate,
};

/// Called by the report window once everything is rendered and painted: prints it to PDF.
pub async fn load_report<R: Runtime>(app: &AppHandle<R>, from: u32, to: u32) -> Result<(), String> {
    let mut config = app
        .config()
        .app
        .windows
        .iter()
        .find(|w| w.label == "report")
        .expect("no 'report' window in tauri.conf.json")
        .clone();

    config.url = tauri::WebviewUrl::App(format!("{}?from={from}&to={to}", config.url).into());

    // Stays hidden: printing starts once the page calls `report_ready`
    WebviewWindowBuilder::from_config(app, &config)
        .map_err(|e| e.to_string())?
        .build()
        .map_err(|e| e.to_string())?;

    Ok(())
}

/// Called by the report window once everything is rendered and painted: prints it to PDF.
pub async fn report_ready(window: WebviewWindow) -> Result<(), String> {
    info!("Report ready, printing");
    let lang = window.state::<SettingsLock>().read().unwrap().language;
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
        .run_in_connection(move |conn| Ok(report_data(conn, from, to)?))
        .await
        .map(|mut data| {
            // Listed by name, which depends on the language
            data.exercises
                .sort_by_key(|e| translate(&format!("exercise_{}_{}", e.category, e.id), lang));
            data
        });

    match res {
        Ok(data) => {
            info!(
                "Retrieved report data: {} sessions, {} exercises, {} body metrics",
                data.sessions.len(),
                data.exercises.len(),
                data.body_metrics.len()
            );
            Ok(data)
        }
        Err(e) => Err(report_error(
            e,
            lang,
            "error_report_data",
            "Error getting report data",
        )),
    }
}

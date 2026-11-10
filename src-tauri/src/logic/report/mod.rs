pub mod commands;
pub mod helpers;
pub mod queries;

use tauri::{AppHandle, Runtime, WebviewWindow, WebviewWindowBuilder};
use tauri_plugin_log::log::info;

use self::helpers::print_to_pdf;
use self::queries::report_data;
use crate::{
    SettingsLock,
    dto::report::ReportData,
    logic::{report_error, run_blocking_conn},
};

/// Called by the report window once everything is rendered and painted: prints it to PDF.
pub async fn load_report<R: Runtime>(app: &AppHandle<R>, from: u32, to: u32) -> Result<(), String> {
    let mut config = app
        .config()
        .app
        .windows
        .iter()
        .find(|w| w.label == "report")
        .expect("no hay ventana 'report' en tauri.conf.json")
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
    let res = print_to_pdf(window.clone()).await;
    // On success the window is closed once the print operation ends
    if res.is_err() {
        let _ = window.close();
    }
    res
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

    let res = run_blocking_conn(app, move |_, conn| report_data(conn, from, to, lang)).await;

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

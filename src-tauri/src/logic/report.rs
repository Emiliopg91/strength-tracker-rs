use std::{collections::BTreeSet, process::Command};

use tauri::{AppHandle, Manager, Runtime, WebviewWindow, WebviewWindowBuilder};
use tauri_plugin_log::log::{error, info};

use crate::{
    SettingsLock,
    dto::report::ReportData,
    logic::{
        ask_save_path,
        body_metrics::fetch_body_measures,
        exercises::fetch_exercise_details,
        report_error, run_blocking,
        sessions::{fetch_session_details, fetch_sessions},
    },
    utils::translations::translate,
};

const STRENGTH_SPORT: u8 = 10;
const STRENGTH_SUB_SPORT: u8 = 20;

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

async fn print_to_pdf(w: WebviewWindow) -> Result<(), String> {
    use webkit2gtk::{PrintOperation, PrintOperationExt};

    let lang = w.state::<SettingsLock>().read().unwrap().language;
    let default_file = w
        .path()
        .download_dir()
        .map_err(|e| e.to_string())?
        .join(format!("{}.pdf", translate("progression_report", lang)));

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

/// Gathers everything the report needs for the `[from, to]` range, reusing the commands' DB logic on one connection.
pub async fn get_report_data<R: Runtime>(
    app: &AppHandle<R>,
    settings: &SettingsLock,
    from: u32,
    to: u32,
) -> Result<ReportData, String> {
    info!("Getting report data...");
    let lang = settings.read().unwrap().language;

    let res = run_blocking(app, move |database| {
        database.run_in_connection(|conn| {
            let mut sessions: Vec<_> = fetch_sessions(
                conn,
                Some(from as i32),
                Some(to as i32),
                Some(STRENGTH_SPORT),
                Some(STRENGTH_SUB_SPORT),
            )?
            .into_iter()
            .collect();
            sessions.reverse();

            let session_details = sessions
                .iter()
                .map(|s| fetch_session_details(conn, s.timestamp, false, false))
                .collect::<Result<Vec<_>, _>>()?;

            // Sorted and deduplicated; sets without exercise are skipped
            let trained: BTreeSet<(u16, u16)> = session_details
                .iter()
                .flat_map(|d| &d.steps)
                .filter_map(|set| Some((set.ex_cat?, set.ex_id?)))
                .collect();

            let mut exercises: Vec<_> = trained
                .into_iter()
                .filter_map(|(category, id)| {
                    if let Ok(mut details) =
                        fetch_exercise_details(conn, category, id, Some((from, to)))
                    {
                        details.workouts.reverse();
                        Some(details)
                    } else {
                        None
                    }
                })
                .collect();
            exercises
                .sort_by_key(|e| translate(&format!("exercise_{}_{}", e.category, e.id), lang));

            let mut body_metrics = fetch_body_measures(conn, Some((from, to)))?;
            body_metrics.reverse();

            Ok(ReportData {
                sessions: session_details,
                exercises,
                body_metrics,
            })
        })
    })
    .await;

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

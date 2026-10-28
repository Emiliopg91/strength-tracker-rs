mod dao;
mod dto;
mod fit;
mod logic;
mod mtp;
mod rclone;
mod udev;
mod utils;

#[cfg(test)]
mod tests;

#[cfg(debug_assertions)]
use std::path::Path;
use std::{fs, process::exit, sync::RwLock, time::Duration};

use rusqlite_orm::database::{
    DatabasePool, DdlVersion,
    builder::{DatabaseConnectionBuilder, JournalMode},
};
use rusqlite_orm::ddls;
use tauri::{
    AppHandle, Manager, RunEvent, WindowEvent,
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
};
use tauri_plugin_log::{
    Target, TargetKind,
    log::{LevelFilter, debug, error, info},
};

use crate::{
    dto::app::Settings,
    logic::{
        app::{
            get_environment, get_settings, get_translations, hide_main_window,
            notify_frontend_ready, show_main_window, update_settings_value,
        },
        body_metrics::{add_body_measures, delete_body_metric, get_body_measures},
        devices::{get_registered_devices, import_from_device, send_to_device},
        exercises::{
            apply_progression_suggestion, get_exercise_details, get_exercises,
            get_exercises_catalog, get_progression_status, get_progression_suggestion,
            save_exercise_notes, set_exercise_progression, set_exercise_progression_parameters,
            set_exercise_unilateral,
        },
        export::{export_database, export_gpx, upload_to_cloud},
        report::{get_report_data, load_report, report_ready},
        sessions::{
            _import_from_files, get_heatmap_data, get_session_details, get_sessions,
            import_from_files, save_session_changes,
        },
        workouts::{
            get_workout_details, get_workout_list, load_workout_page, save_workout,
            set_workout_status, workout_print_ready,
        },
    },
    udev::UdevManager,
    utils::{constants, single_instance::SingleInstance, translations::translate},
};

#[cfg(debug_assertions)]
use crate::fit::parser::FitParser;

#[cfg(debug_assertions)]
pub fn decode_files<P>(paths: &[P])
where
    P: AsRef<Path>,
{
    for path in paths {
        let res: Result<(), Box<dyn std::error::Error>> = match FitParser::from_file(path) {
            Ok(stream) => match stream.debug_dump() {
                Ok(_) => Ok(()),
                Err(e) => Err(e),
            },
            Err(e) => Err(Box::new(e)),
        };
        if let Err(e) = res {
            eprintln!(
                "Error handling '{}': \n  {}",
                path.as_ref().display(),
                e.as_ref()
            )
        }
    }
}

#[cfg(debug_assertions)]
pub fn encode_files<P>(paths: &[P])
where
    P: AsRef<Path>,
{
    for path in paths {
        if let Err(e) = crate::fit::parser::debug_encode(path) {
            eprintln!("Error handling '{}': \n  {}", path.as_ref().display(), e)
        }
    }
}

/// Debug-only incremental dump of the connected Garmin device into `./dump` (`--dump-device`).
#[cfg(debug_assertions)]
pub fn dump_device() {
    if let Err(e) = crate::mtp::dump::run(Path::new("dump")) {
        eprintln!("Error dumping device: {e}");
    }
}

pub fn check_running() -> bool {
    SingleInstance::is_app_running().0
}

pub fn write_mtp_rules(auto_run: bool) -> crate::udev::errors::Result<()> {
    if !fs::exists(constants::RULE_FILE).unwrap() {
        info!("Installing udev rules...");
        UdevManager::write_rules_file(auto_run)?;

        Ok(())
    } else {
        Ok(())
    }
}

pub fn force_write_mtp_rules(auto_run: bool) -> crate::udev::errors::Result<()> {
    UdevManager::write_rules_file(auto_run)
}

pub type SettingsLock = RwLock<Settings>;

fn initialize() -> (DatabasePool, Settings) {
    debug!("Initializing database...");
    let builder = DatabaseConnectionBuilder::default()
        .location(constants::DB_FILE.clone())
        .busy_timeout(Duration::from_secs(8))
        .connection_timeout(Duration::from_secs(5))
        .pool_size(5)
        .min_idle(2)
        .enable_foreign_keys()
        .journal_mode(JournalMode::Delete);
    match builder.build("gtrs") {
        Ok(database) => {
            if let Err(e) = database.create_schema(&schema_ddls()) {
                error!("Could not initialize database: {}", e);
                exit(constants::ExitCodes::DbError.into())
            }

            debug!("Loading settings...");
            let settings = Settings::from(&database);

            crate::dao::settings::Settings::set_version(
                &database,
                &constants::APP_SEM_VERSION.clone(),
            )
            .unwrap();

            //merge_sessions(&database, _, _);

            (database, settings)
        }
        Err(e) => {
            error!("Could not open database: {}", e);
            exit(constants::ExitCodes::DbError.into())
        }
    }
}

/// Schema DDLs sorted by version, with the data update hooks attached to their migration.
fn schema_ddls() -> Vec<DdlVersion> {
    let mut ddls = ddls!("../resources/ddl").to_vec();
    ddls.sort_by_key(|ddl| ddl.version);

    /*for ddl in &mut ddls {
        ddl.update_fn = match ddl.version {
            _ => None,
        };
    }*/

    ddls
}

/// Turns SIGTERM/SIGINT/SIGHUP into a regular `app.exit(0)` so they are logged instead of killing the process.
fn listen_for_signals(app: &AppHandle) {
    use tokio::signal::unix::{SignalKind, signal};

    for (kind, name) in [
        (SignalKind::terminate(), "SIGTERM"),
        (SignalKind::interrupt(), "SIGINT"),
        (SignalKind::hangup(), "SIGHUP"),
    ] {
        let app = app.clone();
        tauri::async_runtime::spawn(async move {
            match signal(kind) {
                Ok(mut stream) => {
                    stream.recv().await;
                    info!("Received {}, shutting down...", name);
                    app.exit(0);
                }
                Err(e) => error!("Could not listen for {}: {}", name, e),
            }
        });
    }
}

/// Boots the Tauri app: acquires the single-instance lock, opens/migrates the DB, loads settings, and registers commands.
pub fn run(log_level: LevelFilter) {
    SingleInstance::acquire();

    tauri::Builder::default()
        .plugin(tauri_plugin_autostart::Builder::new().build())
        .plugin(
            tauri_plugin_log::Builder::new()
                .level(LevelFilter::Warn)
                .level_for(constants::LIB_NAME.clone(), log_level)
                .level_for("command", log_level)
                .level_for("rusqlite_orm", log_level)
                .clear_targets()
                .target(Target::new(TargetKind::Folder {
                    path: constants::LOGS_DIR.clone(),
                    file_name: None,
                }))
                .target(
                    Target::new(TargetKind::Stdout).format(|out, message, _record| {
                        let message = message.to_string();
                        let char_count = message.chars().count();
                        let message = if char_count > 1000 {
                            let truncated: String = message.chars().take(1000).collect();
                            format!("{}... and {} more", truncated, char_count - 1000)
                        } else {
                            message
                        };

                        out.finish(format_args!("{}", message))
                    }),
                )
                .max_file_size(constants::LOG_FILE_MAX_SIZE)
                .rotation_strategy(constants::LOG_FILE_ROTATION_STRATEGY.clone())
                .format(|out, message, record| {
                    let mut target = record.target();
                    target = if target.len() > 30 {
                        &target[target.len() - 30..]
                    } else {
                        target
                    };

                    out.finish(format_args!(
                        "[{}][{:<30}][{:<5.5}] {}",
                        chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f"),
                        target,
                        record.level().to_string(),
                        message
                    ))
                })
                .build(),
        )
        .plugin(tauri_plugin_opener::init())
        .setup(move |app| {
            info!(
                "Starting {} v{} with PID {}",
                *constants::APP_NAME,
                *constants::APP_VERSION,
                *constants::PID
            );

            if let Err(e) = write_mtp_rules(false) {
                eprintln!("Error installing udev rules {e}");
                exit(constants::ExitCodes::UdevError.into())
            }

            let (database, settings) = initialize();

            listen_for_signals(app.handle());

            let app_handle = app.handle().clone();
            let window = app_handle.get_webview_window("main");
            if let Some(window) = window {
                window.on_window_event(move |event| {
                    if let WindowEvent::CloseRequested { api, .. } = event {
                        if app_handle
                            .state::<SettingsLock>()
                            .read()
                            .unwrap()
                            .close_to_tray
                        {
                            api.prevent_close();
                            let _ = hide_main_window(&app_handle);
                        }
                    } else if let WindowEvent::DragDrop(event) = event
                        && let tauri::DragDropEvent::Drop { paths, position: _ } = event
                    {
                        let paths: Vec<_> = paths
                            .iter()
                            .filter(|p| p.display().to_string().to_lowercase().ends_with(".fit"))
                            .cloned()
                            .collect();

                        info!("Dropped {} .fit files: {:?}", paths.len(), paths);

                        let app_handle = app_handle.clone();
                        tauri::async_runtime::spawn_blocking(move || {
                            let _ = _import_from_files(app_handle, paths.as_slice());
                        });
                    }
                });
            }

            let open = MenuItem::with_id(
                app,
                "open",
                translate("open", settings.language),
                true,
                None::<&str>,
            )?;
            let separator = PredefinedMenuItem::separator(app)?;
            let exit = MenuItem::with_id(
                app,
                "exit",
                translate("exit", settings.language),
                true,
                None::<&str>,
            )?;
            let menu = Menu::with_items(app, &[&open, &separator, &exit])?;
            TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
                .tooltip(constants::APP_TITLE)
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "open" => {
                        let _ = show_main_window(app);
                    }
                    "exit" => app.exit(0),
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        let _ = show_main_window(tray.app_handle());
                    }
                })
                .build(app)?;

            app.manage(database);
            app.manage(SettingsLock::new(settings));

            debug!("Setup finished");
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            //Sessions
            get_sessions,
            get_session_details,
            save_session_changes,
            import_from_files,
            get_heatmap_data,
            //Exercises
            get_exercises,
            get_exercise_details,
            get_exercises_catalog,
            set_exercise_unilateral,
            set_exercise_progression,
            get_progression_suggestion,
            apply_progression_suggestion,
            save_exercise_notes,
            set_exercise_progression_parameters,
            get_progression_status,
            //Workouts
            get_workout_list,
            get_workout_details,
            set_workout_status,
            save_workout,
            load_workout_page,
            workout_print_ready,
            //Devices
            import_from_device,
            send_to_device,
            get_registered_devices,
            //App
            notify_frontend_ready,
            get_report_data,
            get_environment,
            get_settings,
            update_settings_value,
            get_translations,
            //Body Metrics
            get_body_measures,
            add_body_measures,
            delete_body_metric,
            //Export
            export_database,
            upload_to_cloud,
            export_gpx,
            //Report
            report_ready,
            load_report
        ])
        .build(tauri::generate_context!())
        .unwrap_or_else(|e| {
            eprintln!("Error while running tauri application {}", e);
            exit(constants::ExitCodes::TauriError.into())
        })
        .run(|app, event| match event {
            RunEvent::ExitRequested { api, code, .. }
                if code.is_none() && app.state::<SettingsLock>().read().unwrap().close_to_tray =>
            {
                api.prevent_exit();
                let _ = hide_main_window(app);
            }
            RunEvent::Exit => info!(
                "Stopped {} v{}",
                *constants::APP_NAME,
                *constants::APP_VERSION
            ),
            _ => {}
        });
}

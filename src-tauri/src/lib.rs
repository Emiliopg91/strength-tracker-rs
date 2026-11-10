mod connect;
mod dao;
#[cfg(debug_assertions)]
pub mod debug;
mod dto;
mod fit;
mod logic;
mod mtp;
mod rclone;
mod udev;
mod utils;

#[cfg(test)]
mod tests;

use std::{process::exit, sync::RwLock, time::Duration};

use rusqlite_orm::database::{
    DatabasePool, DdlVersion,
    builder::{DatabaseConnectionBuilder, JournalMode},
};
use rusqlite_orm::ddls;
use tauri::{
    AppHandle, Emitter, Manager, RunEvent, WindowEvent,
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
};
use tauri_plugin_log::{
    Target, TargetKind,
    log::{LevelFilter, debug, error, info},
};

#[cfg(debug_assertions)]
use crate::logic::exercises::queries::update_all_prs;
use crate::{
    connect::ConnectClient,
    dao::body_metric::migration_21,
    dto::app::Settings,
    logic::{
        app::{
            commands::{
                get_environment, get_settings, get_translations, notify_frontend_ready,
                update_settings_value,
            },
            helpers::{hide_main_window, show_main_window},
        },
        body_metrics::commands::{add_body_measures, delete_body_metric, get_body_measures},
        connect::commands::{
            connect_logged_user, connect_login, connect_logout, import_from_connect,
            import_weight_from_connect, send_to_connect,
        },
        devices::commands::{get_registered_devices, import_from_device, send_to_device},
        exercises::commands::{
            apply_progression_suggestion, available_any_progression_suggestions,
            get_exercise_details, get_exercises, get_exercises_catalog, get_progression_status,
            get_progression_suggestion, save_exercise_notes, set_exercise_progression,
            set_exercise_progression_parameters, set_exercise_unilateral,
        },
        export::commands::{export_database, upload_to_cloud},
        prompts::{
            commands::answer_prompt,
            helpers::{AppPrompter, Prompts},
        },
        report::commands::{get_report_data, load_report, report_ready},
        sessions::{
            commands::{
                download_session, get_heatmap_data, get_session_details, get_sessions,
                import_from_files, save_session_changes,
            },
            helpers::import_file_paths,
        },
        workouts::commands::{
            get_workout_details, get_workout_list, load_workout_page, save_workout,
            set_workout_status, workout_print_ready,
        },
    },
    mtp::MtpClient,
    udev::UdevManager,
    utils::{constants, single_instance::SingleInstance, translations::translate},
};

pub fn check_running() -> bool {
    SingleInstance::is_app_running().0
}

pub fn force_write_mtp_rules(auto_run: bool) -> crate::udev::errors::Result<()> {
    UdevManager::write_rules_file(auto_run)
}

pub type SettingsLock = RwLock<Settings>;
pub type ConnectLock = tokio::sync::Mutex<ConnectClient>;
pub type MtpLock = tokio::sync::Mutex<MtpClient>;

fn initialize(app: &AppHandle) -> (DatabasePool, Settings, ConnectClient, MtpClient) {
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

            //crate::logic::sessions::queries::merge_sessions(&database, _, _);

            #[cfg(debug_assertions)]
            let _ = database.run_in_transaction(|tx| Ok(update_all_prs(tx)?));

            (
                database,
                settings,
                {
                    let app: AppHandle = app.clone();
                    let prompter = AppPrompter(app.clone());
                    ConnectClient::new(
                        move |user| {
                            // Typed binding: the TypeScript generator reads the payload type from it
                            let payload: Option<String> = user;
                            let _ = app.emit("connect_logged", payload);
                        },
                        prompter,
                    )
                },
                MtpClient {},
            )
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

    for ddl in &mut ddls {
        ddl.update_fn = match ddl.version {
            21 => Some(|tx| migration_21(tx)),
            _ => None,
        };
    }

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
        .plugin(tauri_plugin_dialog::init())
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

            let (database, settings, connect, mtp_client) = initialize(app.handle());

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
                            let _ = import_file_paths(&app_handle, paths.as_slice());
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

            app.manage(Prompts::default());
            app.manage(database);
            app.manage(SettingsLock::new(settings));
            app.manage(ConnectLock::new(connect));
            app.manage(MtpLock::new(mtp_client));

            debug!("Setup finished");
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            //Prompts
            answer_prompt,
            //Sessions
            get_sessions,
            get_session_details,
            download_session,
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
            available_any_progression_suggestions,
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
            //Report
            report_ready,
            load_report,
            //Connect
            import_from_connect,
            import_weight_from_connect,
            connect_logged_user,
            connect_logout,
            connect_login,
            send_to_connect,
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

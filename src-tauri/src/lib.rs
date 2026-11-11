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
use tauri::{AppHandle, Emitter, Manager, RunEvent, WindowEvent};
use tauri_plugin_log::{
    Target, TargetKind,
    log::{LevelFilter, debug, error, info, warn},
};

#[cfg(debug_assertions)]
use crate::logic::exercises::queries::update_all_prs;
use crate::{
    connect::ConnectClient,
    dao::body_metric::migration_21,
    dto::app::Settings,
    logic::{
        app::commands::{
            get_environment, get_settings, get_translations, notify_frontend_ready,
            update_settings_value,
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
    utils::{constants, single_instance::SingleInstance},
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
            // `setup` is synchronous and runs outside the async runtime, so it can wait here
            let settings = tauri::async_runtime::block_on(async {
                if let Err(e) = database.create_schema(&schema_ddls()).await {
                    error!("Could not initialize database: {}", e);
                    exit(constants::ExitCodes::DbError.into())
                }
                // Keeps the planner statistics current, analyzing only the tables that need it
                if let Err(e) = database
                    .run_in_connection(|conn| Ok(conn.execute_batch("PRAGMA optimize;")?))
                    .await
                {
                    warn!("Could not optimize database: {}", e);
                }

                debug!("Loading settings...");
                let settings = Settings::load(&database).await;

                crate::dao::settings::Settings::set(
                    &database,
                    crate::dao::settings::settings_keys::VERSION,
                    &*constants::APP_SEM_VERSION,
                )
                .await
                .unwrap();

                //crate::logic::sessions::queries::merge_sessions(&database, _, _);

                #[cfg(debug_assertions)]
                let _ = database
                    .run_in_transaction(|tx| Ok(update_all_prs(tx)?))
                    .await;

                settings
            });

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
                    if let WindowEvent::DragDrop(event) = event
                        && let tauri::DragDropEvent::Drop { paths, position: _ } = event
                    {
                        let paths: Vec<_> = paths
                            .iter()
                            .filter(|p| p.display().to_string().to_lowercase().ends_with(".fit"))
                            .cloned()
                            .collect();

                        info!("Dropped {} .fit files: {:?}", paths.len(), paths);

                        let app_handle = app_handle.clone();
                        tauri::async_runtime::spawn(async move {
                            let _ = import_file_paths(&app_handle, paths).await;
                        });
                    }
                });
            }

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
        .run(|_, event| {
            if let RunEvent::Exit = event {
                info!(
                    "Stopped {} v{}",
                    *constants::APP_NAME,
                    *constants::APP_VERSION
                )
            }
        });
}

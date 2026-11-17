// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::process::exit;
use strength_coach_for_garmin_lib::{check_running, force_write_mtp_rules};
use tauri_plugin_log::log::LevelFilter;

/// Binary entrypoint: handles the debug-only options, sets up env vars, and hands off to `run()`.
fn main() {
    let args: Vec<_> = std::env::args().collect();

    match args.get(1).map(String::as_str) {
        Some("--poll") => exit(i32::from(check_running())),
        Some("--rules") => {
            if let Some(auto_run) = args.get(2) {
                exit(match force_write_mtp_rules(auto_run == "true") {
                    Ok(()) => 0,
                    Err(e) => {
                        eprintln!("Error writing udev rules: {e}");
                        1
                    }
                })
            }
            eprintln!("Bad usage");
        }
        Some("--on-connect") => {}
        _ => {}
    }

    #[cfg(debug_assertions)]
    {
        if let Some(code) = strength_coach_for_garmin_lib::debug::run_option(&args) {
            exit(code);
        }
        if std::env::var("IN_DEBUG").is_err() {
            tauri_rs_ts_ipc::build();
        }
    }

    let log_level: LevelFilter = if args.iter().any(|a| a == "-v") {
        LevelFilter::Debug
    } else {
        LevelFilter::Info
    };

    strength_coach_for_garmin_lib::run(log_level);
}

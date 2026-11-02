// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::process::exit;
use strength_tracker_rs_lib::{check_running, force_write_mtp_rules};
use tauri_plugin_log::log::LevelFilter;

/// Binary entrypoint: handles the debug-only options, sets up env vars, and hands off to `run()`.
fn main() {
    let args: Vec<_> = std::env::args().collect();

    if args.len() > 1 {
        match args.get(1).unwrap().as_str() {
            "--poll" => {
                exit(if check_running() { 1 } else { 0 });
            }
            "--rules" => {
                if args.len() > 2 {
                    let auto_run = args.get(2).unwrap() == "true";
                    exit(match force_write_mtp_rules(auto_run) {
                        Ok(()) => 0,
                        Err(e) => {
                            eprintln!("Error writing udev rules: {e}");
                            1
                        }
                    })
                } else {
                    eprintln!("Bad usage")
                }
            }
            "--on-connect" => {}
            _ => {}
        }
    }

    #[cfg(debug_assertions)]
    {
        if let Some(code) = strength_tracker_rs_lib::debug::run_option(&args) {
            exit(code);
        }
        if std::env::var("IN_DEBUG").is_err() {
            tauri_rs_ts_ipc::build();
        }
    }

    let log_level: LevelFilter = if args.contains(&"-v".to_string()) {
        LevelFilter::Debug
    } else {
        LevelFilter::Info
    };

    strength_tracker_rs_lib::run(log_level);
}

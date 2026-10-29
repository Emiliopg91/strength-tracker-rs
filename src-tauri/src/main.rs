// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use tauri_plugin_log::log::LevelFilter;

fn main() {
    strength_tracker_rs_lib::run(LevelFilter::Info);
}

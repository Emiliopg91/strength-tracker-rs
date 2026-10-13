use std::{fs, path::PathBuf, sync::LazyLock};

use semver::Version;
use tauri_plugin_log::RotationStrategy;

use crate::utils::translations::Languages;

// App block
pub static APP_TITLE: &str = "Strength Tracker";
pub static APP_NAME: LazyLock<String> = LazyLock::new(|| env!("CARGO_PKG_NAME").to_string());
pub static APP_VERSION: LazyLock<String> = LazyLock::new(|| env!("CARGO_PKG_VERSION").to_string());
pub static APP_SEM_VERSION: LazyLock<Version> =
    LazyLock::new(|| Version::parse(&APP_VERSION).unwrap());
pub static LIB_NAME: LazyLock<String> =
    LazyLock::new(|| format!("{}_lib", APP_NAME.replace('-', "_")));
pub static PID: LazyLock<u32> = LazyLock::new(std::process::id);
pub static LOCK_FILE: LazyLock<PathBuf> = LazyLock::new(|| {
    let run_dir = std::env::var("XDG_RUNTIME_DIR").expect("Could not get runtime dir");
    PathBuf::from(run_dir).join(format!("{}.lock", *APP_NAME))
});
pub static URL: &str =
    "https://api.github.com/repos/Emiliopg91/strength-tracker-rs/releases/latest";

pub static RULE_FILE: &str = "/etc/udev/rules.d/99-strength-tracker-rs.rules";
/// Rules file written by releases prior to the rename, removed when the new one is written.
pub static LEGACY_RULE_FILE: &str = "/etc/udev/rules.d/99-garmin-tracker-rs.rules";

// MTP block
pub static MTP_GARMIN_MANUFACTURER: &str = "GARMIN";
pub const MTP_GARMIN_VENDOR_ID: u16 = 0x091e;
pub static MTP_GARMIN_ROOT_FOLDER: &str = "GARMIN";
pub static MTP_GARMIN_ACTIVITY_FOLDER: &str = "Activity";
pub static MTP_GARMIN_WORKOUTS_FOLDER: &str = "Workouts";
pub static MTP_GARMIN_NEW_FILES_FOLDER: &str = "NewFiles";
pub static MTP_TMP_DIR_PREFIX: &str = "strength-tracker-rs";

// Languages block
pub static DEFAULT_LANGUAGE: Languages = Languages::English;
pub static SYSTEM_LANGUAGE: LazyLock<Languages> = LazyLock::new(|| {
    let mut lang_var = std::env::var("LANG").unwrap_or("C".to_string());
    lang_var = lang_var.to_lowercase();
    if lang_var == "c" {
        lang_var = Languages::English.to_string();
    }
    if lang_var.contains(".") {
        lang_var = lang_var.split(".").next().unwrap().into();
    }
    if lang_var.contains("_") {
        lang_var = lang_var.split("_").next().unwrap().into();
    }

    Languages::from(&lang_var)
});

// Dir block
pub static HOME_DIR: LazyLock<PathBuf> =
    LazyLock::new(|| PathBuf::from(std::env::var("HOME").expect("Could not get home folder")));
pub static DATA_LOCAL_DIR: LazyLock<PathBuf> = LazyLock::new(|| {
    let share_dir = HOME_DIR.join(".local").join("share");
    let dir = share_dir.join(APP_NAME.as_str());
    migrate_legacy_dir(&share_dir.join(LEGACY_APP_NAME), &dir);
    ensure_dir(dir)
});
/// Package name used before the rename, kept to migrate existing user data.
static LEGACY_APP_NAME: &str = "garmin-tracker-rs";

// Database block
pub static DB_FILE: LazyLock<PathBuf> = LazyLock::new(|| DATA_LOCAL_DIR.join("database.db"));

// Logs block
pub static LOGS_DIR: LazyLock<PathBuf> = LazyLock::new(|| ensure_dir(DATA_LOCAL_DIR.join("logs")));
pub static LOG_FILE_MAX_SIZE: u128 = 1 * 1_024 * 1_024;
pub static LOG_FILE_ROTATION_STRATEGY: RotationStrategy = RotationStrategy::KeepSome(3);

pub static RCLONE_CONFIG_NAME: LazyLock<String> = LazyLock::new(|| APP_NAME.to_string());
pub static RCLONE_CONFIG_FILE: LazyLock<PathBuf> =
    LazyLock::new(|| HOME_DIR.join(".config").join("rclone").join("rclone.conf"));

#[repr(i32)]
pub enum ExitCodes {
    UdevError = 1,
    DbError = 2,
    SettingsError = 3,
    NoMainWindow = 4,
    TauriError = 5,
}

impl From<ExitCodes> for i32 {
    fn from(val: ExitCodes) -> Self {
        val as i32
    }
}

// UI block
pub static ICON_PATH: LazyLock<String> = LazyLock::new(|| {
    #[cfg(debug_assertions)]
    {
        std::env::current_exe()
            .unwrap()
            .parent()
            .unwrap()
            .join("../../icons/icon.png")
            .display()
            .to_string()
    }

    #[cfg(not(debug_assertions))]
    {
        "/usr/share/icons/hicolor/128x128/apps/strength-tracker-rs.png".to_string()
    }
});

/// Moves the pre-rename data dir to its new location, unless the new one already exists.
fn migrate_legacy_dir(legacy: &PathBuf, dir: &PathBuf) {
    if legacy.is_dir() && !dir.exists() {
        fs::rename(legacy, dir).unwrap_or_else(|e| {
            panic!(
                "Could not migrate {} to {}: {e}",
                legacy.display(),
                dir.display()
            )
        });
    }
}

/// Creates `dir` (and parents) if it doesn't exist yet, then returns it.
fn ensure_dir(dir: PathBuf) -> PathBuf {
    if !dir.exists() {
        fs::create_dir_all(&dir)
            .unwrap_or_else(|e| panic!("Could not create directory {}: {e}", dir.display()));
    }
    dir
}

use rusqlite_orm::Entity;
use rusqlite_orm::{dao::Repository, database::DatabasePool};
use semver::Version;
use serde::{Deserialize, Serialize};
use std::fmt;

use crate::utils::{constants, translations::Languages};

pub mod settings_keys {
    pub const DISTANCE_UNIT: &str = "distance_unit";
    pub const LANGUAGE: &str = "language";
    pub const START_ON_BOOT: &str = "start_boot";
    pub const WEIGHT_UNIT: &str = "weight_unit";
    pub const ON_DEVICE_CONNECT: &str = "on_device_connect";
    pub const VERSION: &str = "version";
    pub const START_INTO_TRAY: &str = "start_into_tray";
    pub const CLOSE_TO_TRAY: &str = "close_to_tray";
}

#[derive(Entity, Serialize, Deserialize)]
#[primary_key(name)]
pub struct Settings {
    pub name: String,
    pub value: String,
}

impl Settings {
    /// Reads the persisted weight unit, defaulting to kilograms if unset/invalid.
    pub fn get_weight_unit(db: &DatabasePool) -> WeightUnit {
        SettingsRepository::select_by_id(db, settings_keys::WEIGHT_UNIT)
            .ok()
            .flatten()
            .and_then(|r| WeightUnit::try_from(r.value.as_str()).ok())
            .unwrap_or(WeightUnit::Kilograms)
    }
    /// Persists the weight unit setting.
    pub fn set_weight_unit(
        db: &DatabasePool,
        value: &WeightUnit,
    ) -> rusqlite_orm::errors::Result<()> {
        SettingsRepository::insert()
            .or_replace()
            .item(&mut Settings {
                name: settings_keys::WEIGHT_UNIT.to_string(),
                value: value.to_string(),
            })
            .execute(db)
            .map(|_| ())
    }

    /// Reads the persisted distance unit, defaulting to kilometers if unset/invalid.
    pub fn get_distance_unit(db: &DatabasePool) -> DistanceUnit {
        SettingsRepository::select_by_id(db, settings_keys::DISTANCE_UNIT)
            .ok()
            .flatten()
            .and_then(|r| DistanceUnit::try_from(r.value.as_str()).ok())
            .unwrap_or(DistanceUnit::Kilometers)
    }
    /// Persists the distance unit setting.
    pub fn set_distance_unit(
        db: &DatabasePool,
        value: &DistanceUnit,
    ) -> rusqlite_orm::errors::Result<()> {
        SettingsRepository::insert()
            .or_replace()
            .item(&mut Settings {
                name: settings_keys::DISTANCE_UNIT.to_string(),
                value: value.to_string(),
            })
            .execute(db)
            .map(|_| ())
    }

    /// Reads the persisted UI language, defaulting to the system language if unset.
    pub fn get_language(db: &DatabasePool) -> Languages {
        SettingsRepository::select_by_id(db, settings_keys::LANGUAGE)
            .ok()
            .flatten()
            .map(|r| Languages::from(r.value.as_str()))
            .unwrap_or(*constants::SYSTEM_LANGUAGE)
    }
    /// Persists the UI language setting.
    pub fn set_language(db: &DatabasePool, value: &Languages) -> rusqlite_orm::errors::Result<()> {
        SettingsRepository::insert()
            .or_replace()
            .item(&mut Settings {
                name: settings_keys::LANGUAGE.to_string(),
                value: value.to_string(),
            })
            .execute(db)
            .map(|_| ())
    }

    /// Reads whether auto-sync on device connect is enabled, defaulting to `true` if unset.
    pub fn get_on_device_connect(db: &DatabasePool) -> bool {
        SettingsRepository::select_by_id(db, settings_keys::ON_DEVICE_CONNECT)
            .ok()
            .flatten()
            .and_then(|r| r.value.parse().ok())
            .unwrap_or(false)
    }
    /// Persists the auto-sync setting.
    pub fn set_on_device_connect(
        db: &DatabasePool,
        value: bool,
    ) -> rusqlite_orm::errors::Result<()> {
        SettingsRepository::insert()
            .or_replace()
            .item(&mut Settings {
                name: settings_keys::ON_DEVICE_CONNECT.to_string(),
                value: value.to_string(),
            })
            .execute(db)
            .map(|_| ())
    }

    /// Reads whether launch-on-boot is enabled, defaulting to `false` if unset.
    pub fn get_start_on_boot(db: &DatabasePool) -> bool {
        SettingsRepository::select_by_id(db, settings_keys::START_ON_BOOT)
            .ok()
            .flatten()
            .and_then(|r| r.value.parse().ok())
            .unwrap_or(false)
    }
    /// Persists the launch-on-boot setting.
    pub fn set_start_on_boot(db: &DatabasePool, value: bool) -> rusqlite_orm::errors::Result<()> {
        SettingsRepository::insert()
            .or_replace()
            .item(&mut Settings {
                name: settings_keys::START_ON_BOOT.to_string(),
                value: value.to_string(),
            })
            .execute(db)
            .map(|_| ())
    }

    /// Reads whether close to tray is enabled, defaulting to `false` if unset.
    pub fn get_close_to_tray(db: &DatabasePool) -> bool {
        SettingsRepository::select_by_id(db, settings_keys::CLOSE_TO_TRAY)
            .ok()
            .flatten()
            .and_then(|r| r.value.parse().ok())
            .unwrap_or(false)
    }
    /// Persists the close to tray setting.
    pub fn set_close_to_tray(db: &DatabasePool, value: bool) -> rusqlite_orm::errors::Result<()> {
        SettingsRepository::insert()
            .or_replace()
            .item(&mut Settings {
                name: settings_keys::CLOSE_TO_TRAY.to_string(),
                value: value.to_string(),
            })
            .execute(db)
            .map(|_| ())
    }

    /// Reads whether start into tray is enabled, defaulting to `false` if unset.
    pub fn get_start_into_tray(db: &DatabasePool) -> bool {
        SettingsRepository::select_by_id(db, settings_keys::START_INTO_TRAY)
            .ok()
            .flatten()
            .and_then(|r| r.value.parse().ok())
            .unwrap_or(false)
    }
    /// Persists the start into tray setting.
    pub fn set_start_into_tray(db: &DatabasePool, value: bool) -> rusqlite_orm::errors::Result<()> {
        SettingsRepository::insert()
            .or_replace()
            .item(&mut Settings {
                name: settings_keys::START_INTO_TRAY.to_string(),
                value: value.to_string(),
            })
            .execute(db)
            .map(|_| ())
    }

    #[allow(dead_code)]
    /// Reads latest version.
    pub fn get_version(db: &DatabasePool) -> Version {
        SettingsRepository::select_by_id(db, settings_keys::VERSION)
            .ok()
            .flatten()
            .and_then(|r| Version::parse(&r.value).ok())
            .unwrap_or(Version::new(0, 1, 0))
    }
    /// Persists the version.
    pub fn set_version(db: &DatabasePool, value: &Version) -> rusqlite_orm::errors::Result<()> {
        SettingsRepository::insert()
            .or_replace()
            .item(&mut Settings {
                name: settings_keys::VERSION.to_string(),
                value: value.to_string(),
            })
            .execute(db)
            .map(|_| ())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DistanceUnit {
    Kilometers,
    Miles,
}

impl TryFrom<&str> for DistanceUnit {
    type Error = String;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "Kilometers" => Ok(DistanceUnit::Kilometers),
            "Miles" => Ok(DistanceUnit::Miles),
            other => Err(format!("Invalid DistanceUnit '{other}'")),
        }
    }
}

impl fmt::Display for DistanceUnit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            DistanceUnit::Kilometers => "Kilometers",
            DistanceUnit::Miles => "Miles",
        };
        write!(f, "{s}")
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum WeightUnit {
    Kilograms,
    Pounds,
}

impl TryFrom<&str> for WeightUnit {
    type Error = String;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "Kilograms" => Ok(WeightUnit::Kilograms),
            "Pounds" => Ok(WeightUnit::Pounds),
            other => Err(format!("Invalid WeightUnit '{other}'")),
        }
    }
}

impl fmt::Display for WeightUnit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            WeightUnit::Kilograms => "Kilograms",
            WeightUnit::Pounds => "Pounds",
        };
        write!(f, "{s}")
    }
}

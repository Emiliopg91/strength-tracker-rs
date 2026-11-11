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
    /// Stored value of the setting `name`, `None` if unset or unreadable.
    async fn value(db: &DatabasePool, name: &str) -> Option<String> {
        SettingsRepository::select_by_id(db, name)
            .await
            .ok()
            .flatten()
            .map(|r| r.value)
    }

    /// Persists `value` as the setting `name`.
    pub async fn set(
        db: &DatabasePool,
        name: &str,
        value: impl fmt::Display,
    ) -> rusqlite_orm::errors::Result<()> {
        SettingsRepository::insert()
            .or_replace()
            .item(&mut Settings {
                name: name.to_string(),
                value: value.to_string(),
            })
            .execute(db)
            .await
            .map(|_| ())
    }

    /// Reads the persisted weight unit, defaulting to kilograms if unset/invalid.
    pub async fn get_weight_unit(db: &DatabasePool) -> WeightUnit {
        Self::value(db, settings_keys::WEIGHT_UNIT)
            .await
            .and_then(|v| WeightUnit::try_from(v.as_str()).ok())
            .unwrap_or(WeightUnit::Kilograms)
    }

    /// Reads the persisted distance unit, defaulting to kilometers if unset/invalid.
    pub async fn get_distance_unit(db: &DatabasePool) -> DistanceUnit {
        Self::value(db, settings_keys::DISTANCE_UNIT)
            .await
            .and_then(|v| DistanceUnit::try_from(v.as_str()).ok())
            .unwrap_or(DistanceUnit::Kilometers)
    }

    /// Reads the persisted UI language, defaulting to the system language if unset.
    pub async fn get_language(db: &DatabasePool) -> Languages {
        Self::value(db, settings_keys::LANGUAGE)
            .await
            .map(|v| Languages::from(v.as_str()))
            .unwrap_or(*constants::SYSTEM_LANGUAGE)
    }

    /// Reads whether auto-sync on device connect is enabled, defaulting to `false` if unset.
    pub async fn get_on_device_connect(db: &DatabasePool) -> bool {
        Self::flag(db, settings_keys::ON_DEVICE_CONNECT).await
    }

    /// Reads whether launch-on-boot is enabled, defaulting to `false` if unset.
    pub async fn get_start_on_boot(db: &DatabasePool) -> bool {
        Self::flag(db, settings_keys::START_ON_BOOT).await
    }

    /// Reads whether close to tray is enabled, defaulting to `false` if unset.
    pub async fn get_close_to_tray(db: &DatabasePool) -> bool {
        Self::flag(db, settings_keys::CLOSE_TO_TRAY).await
    }

    /// Reads whether start into tray is enabled, defaulting to `false` if unset.
    pub async fn get_start_into_tray(db: &DatabasePool) -> bool {
        Self::flag(db, settings_keys::START_INTO_TRAY).await
    }

    /// Boolean setting `name`, `false` if unset/invalid.
    async fn flag(db: &DatabasePool, name: &str) -> bool {
        Self::value(db, name)
            .await
            .and_then(|v| v.parse().ok())
            .unwrap_or(false)
    }

    #[allow(dead_code)]
    /// Reads latest version.
    pub async fn get_version(db: &DatabasePool) -> Version {
        Self::value(db, settings_keys::VERSION)
            .await
            .and_then(|v| Version::parse(&v).ok())
            .unwrap_or(Version::new(0, 1, 0))
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

use rusqlite_orm::{dao::Repository, database::DatabasePool};
use serde::{Deserialize, Serialize};

#[derive(Serialize)]
pub enum AppEnvironment {
    Debug,
    Release,
}

use crate::{
    dao::settings::{self, DistanceUnit, SettingsRepository, WeightUnit, settings_keys},
    utils::translations::Languages,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub distance_unit: DistanceUnit,
    pub weight_unit: WeightUnit,
    pub start_boot: bool,
    pub language: Languages,
    pub on_device_connect: bool,
    pub close_to_tray: bool,
    pub start_into_tray: bool,
}

impl Settings {
    /// Reads every persisted setting, falling back to its default when unset.
    pub async fn load(database: &DatabasePool) -> Self {
        Self {
            distance_unit: settings::Settings::get_distance_unit(database).await,
            language: settings::Settings::get_language(database).await,
            start_boot: settings::Settings::get_start_on_boot(database).await,
            weight_unit: settings::Settings::get_weight_unit(database).await,
            on_device_connect: settings::Settings::get_on_device_connect(database).await,
            close_to_tray: settings::Settings::get_close_to_tray(database).await,
            start_into_tray: settings::Settings::get_start_into_tray(database).await,
        }
    }

    /// Persists every setting, all or none.
    pub async fn save(&self, database: &DatabasePool) -> rusqlite_orm::errors::Result<()> {
        let values = [
            (settings_keys::DISTANCE_UNIT, self.distance_unit.to_string()),
            (settings_keys::WEIGHT_UNIT, self.weight_unit.to_string()),
            (settings_keys::START_ON_BOOT, self.start_boot.to_string()),
            (settings_keys::LANGUAGE, self.language.to_string()),
            (
                settings_keys::ON_DEVICE_CONNECT,
                self.on_device_connect.to_string(),
            ),
            (settings_keys::CLOSE_TO_TRAY, self.close_to_tray.to_string()),
            (
                settings_keys::START_INTO_TRAY,
                self.start_into_tray.to_string(),
            ),
        ];
        database
            .run_in_transaction(move |tx| {
                for (name, value) in values {
                    SettingsRepository::insert()
                        .or_replace()
                        .item(&mut settings::Settings {
                            name: name.to_string(),
                            value,
                        })
                        .execute_in(tx)?;
                }
                Ok(())
            })
            .await
    }
}

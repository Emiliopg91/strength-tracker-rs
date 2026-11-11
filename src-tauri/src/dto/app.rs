use rusqlite_orm::database::DatabasePool;
use serde::{Deserialize, Serialize};

#[derive(Serialize)]
pub enum AppEnvironment {
    Debug,
    Release,
}

use crate::{
    dao::settings::{DistanceUnit, WeightUnit},
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
            distance_unit: crate::dao::settings::Settings::get_distance_unit(database).await,
            language: crate::dao::settings::Settings::get_language(database).await,
            start_boot: crate::dao::settings::Settings::get_start_on_boot(database).await,
            weight_unit: crate::dao::settings::Settings::get_weight_unit(database).await,
            on_device_connect: crate::dao::settings::Settings::get_on_device_connect(database)
                .await,
            close_to_tray: crate::dao::settings::Settings::get_close_to_tray(database).await,
            start_into_tray: crate::dao::settings::Settings::get_start_into_tray(database).await,
        }
    }
}

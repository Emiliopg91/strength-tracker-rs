use mtp_rs::DeviceInfo;
use rusqlite_orm::Entity;
use serde::{Deserialize, Serialize};

use crate::utils::constants;

#[derive(Clone, Entity, Serialize, Deserialize)]
#[primary_key(serial)]
pub struct Device {
    pub serial: String,
    pub model: String,
    pub last_activity_sync: Option<u32>,
    pub last_body_metric_sync: Option<u32>,
}

impl TryFrom<&DeviceInfo> for Device {
    type Error = ();

    fn try_from(info: &DeviceInfo) -> Result<Self, Self::Error> {
        if info.manufacturer.to_uppercase() != constants::MTP_GARMIN_MANUFACTURER {
            return Err(());
        }

        Ok(Device {
            serial: info.serial_number.clone(),
            model: info.model.clone(),
            last_activity_sync: None,
            last_body_metric_sync: None,
        })
    }
}

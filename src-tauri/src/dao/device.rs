use std::num::ParseIntError;

use mtp_rs::DeviceInfo;
use rusqlite_orm::Entity;
use serde::{Deserialize, Serialize};

use crate::{connect::client::DeviceSummary, utils::constants};

#[derive(Clone, Entity, Serialize, Deserialize)]
#[primary_key(serial)]
pub struct Device {
    pub serial: String,
    pub model: String,
    pub last_activity_sync: Option<u32>,
    pub last_body_metric_sync: Option<u32>,
    pub usb: bool,
    pub connect: bool,
    pub strength_workout: bool,
}

impl Device {
    pub fn serial_from_unit_id(unit_id: u64) -> String {
        format!("{unit_id:012x}")
    }

    pub fn unit_id_from_serial(serial: &str) -> Result<u64, ParseIntError> {
        u64::from_str_radix(serial, 16)
    }
}

impl TryFrom<&DeviceInfo> for Device {
    type Error = ();

    fn try_from(info: &DeviceInfo) -> Result<Self, Self::Error> {
        if info.manufacturer.to_uppercase() != constants::MTP_GARMIN_MANUFACTURER {
            return Err(());
        }

        Ok(Device {
            serial: info.serial_number.clone(),
            model: info.model.to_uppercase(),
            last_activity_sync: None,
            last_body_metric_sync: None,
            usb: true,
            connect: false,
            strength_workout: true,
        })
    }
}

impl From<&DeviceSummary> for Device {
    fn from(info: &DeviceSummary) -> Self {
        Device {
            serial: Device::serial_from_unit_id(info.unit_id),
            model: info.display_name.to_uppercase(),
            last_activity_sync: None,
            last_body_metric_sync: None,
            usb: false,
            connect: true,
            strength_workout: info.strength_workout_capable,
        }
    }
}

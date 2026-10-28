use serde::Serialize;

use crate::dao::device::Device;

#[derive(Serialize, Clone)]
pub struct DeviceListItem {
    pub manufacturer: String,
    pub model: String,
    pub serial_number: String,
}

impl From<&Device> for DeviceListItem {
    fn from(value: &Device) -> Self {
        Self {
            manufacturer: "Garmin".to_string(),
            model: value.model.clone(),
            serial_number: value.serial.clone(),
        }
    }
}

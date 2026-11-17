use std::{
    collections::{BTreeSet, HashSet},
    num::ParseIntError,
};

use mtp_rs::DeviceInfo;
use rusqlite_orm::{Entity, dao::Repository, rusqlite::Connection, types::where_clause::Where};
use serde::{Deserialize, Serialize};

use crate::{connect::client::DeviceSummary, dao::session::Session, utils::constants};

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

    /// Enrolls the workout-capable Connect devices in `listed`, refreshes the Connect-owned fields of
    /// known ones and clears the `connect` flag of those no longer listed. Returns the added or changed
    /// devices, to be announced.
    pub async fn sync_connect(
        tx: &rusqlite_orm::rusqlite::Transaction<'_>,
        listed: &[Device],
    ) -> rusqlite_orm::errors::Result<Vec<Device>> {
        let mut discovered = Vec::new();
        for mut dao in listed.iter().cloned() {
            match DeviceRepository::select_by_id_in(tx, &dao.serial).await? {
                None => {
                    DeviceRepository::insert()
                        .item(&mut dao)
                        .execute_in(tx)
                        .await?;
                }
                Some(db)
                    if !db.connect
                        || db.model != dao.model
                        || db.strength_workout != dao.strength_workout =>
                {
                    DeviceRepository::update()
                        .set(entity::columns::CONNECT, true.into())
                        .set(entity::columns::MODEL, dao.model.clone().into())
                        .set(
                            entity::columns::STRENGTH_WORKOUT,
                            dao.strength_workout.into(),
                        )
                        .where_(Where::Eq(
                            entity::columns::SERIAL,
                            dao.serial.clone().into(),
                        ))
                        .execute_in(tx)
                        .await?;
                }
                Some(_) => {}
            }

            discovered.push(dao);
        }

        let removed: Vec<Device> = DeviceRepository::select()
            .where_(Where::Eq(entity::columns::CONNECT, true.into()))
            .fetch_in(tx)
            .await?
            .into_iter()
            .filter(|db| !listed.iter().any(|d| d.serial == db.serial))
            .collect();

        if !removed.is_empty() {
            DeviceRepository::update()
                .set(entity::columns::CONNECT, false.into())
                .where_(Where::In(
                    entity::columns::SERIAL,
                    removed.iter().map(|d| d.serial.clone().into()).collect(),
                ))
                .execute_in(tx)
                .await?;
        }

        Ok(discovered)
    }

    /// The Connect pseudo-device and the dates of every stored session, to import only new activities.
    pub async fn get_connect_import_state(
        conn: &Connection,
    ) -> rusqlite_orm::errors::Result<(Device, BTreeSet<u32>)> {
        let device = DeviceRepository::select()
            .where_(Where::Eq(
                entity::columns::SERIAL,
                constants::CONNECT_DEVICE_SERIAL.into(),
            ))
            .fetch_one_in(conn)
            .await?
            .unwrap();

        let existing: BTreeSet<u32> = Session::get_dates(conn).await?;

        Ok((device, existing))
    }

    /// Registers a device plugged by USB, or flags a known one as reachable by USB.
    pub async fn enrol_usb(
        tx: &rusqlite_orm::rusqlite::Transaction<'_>,
        device: &mut Device,
    ) -> rusqlite_orm::errors::Result<()> {
        let _: () =
            if let Some(device) = DeviceRepository::select_by_id_in(tx, &device.serial).await? {
                if !device.usb {
                    DeviceRepository::update()
                        .set(entity::columns::USB, true.into())
                        .where_(Where::Eq(
                            entity::columns::SERIAL,
                            device.serial.clone().into(),
                        ))
                        .execute_in(tx)
                        .await
                        .map(|_| ())?
                }
            } else {
                DeviceRepository::insert()
                    .item(device)
                    .execute_in(tx)
                    .await
                    .map(|_| ())?
            };
        Ok(())
    }

    /// The registered device with that serial number, `None` if it isn't registered.
    pub async fn get(
        conn: &Connection,
        serial: &str,
    ) -> rusqlite_orm::errors::Result<Option<Device>> {
        DeviceRepository::select_by_id_in(conn, serial).await
    }

    /// Every registered device, the Connect pseudo-device included.
    pub async fn get_all(conn: &Connection) -> rusqlite_orm::errors::Result<Vec<Device>> {
        DeviceRepository::select().fetch_in(conn).await
    }

    /// The registered devices among `serials`.
    pub async fn get_by_serials(
        conn: &Connection,
        serials: &HashSet<String>,
    ) -> rusqlite_orm::errors::Result<Vec<Device>> {
        if serials.is_empty() {
            return Ok(Vec::new());
        }
        DeviceRepository::select()
            .where_(Where::In(
                entity::columns::SERIAL,
                serials.iter().map(|s| s.clone().into()).collect(),
            ))
            .fetch_in(conn)
            .await
    }

    /// Serial numbers of every registered device.
    pub async fn get_serials(conn: &Connection) -> rusqlite_orm::errors::Result<HashSet<String>> {
        Ok(DeviceRepository::select()
            .columns(&[entity::columns::SERIAL])
            .fetch_in(conn)
            .await?
            .iter()
            .map(|row| row.get_text(entity::columns::SERIAL.as_ref()))
            .collect())
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

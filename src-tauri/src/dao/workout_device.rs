use std::collections::HashSet;

use rusqlite_orm::{
    Entity,
    dao::Repository,
    rusqlite::Connection,
    types::{order_by::OrderBy, where_clause::Where},
};

use crate::{
    dao::{
        device::{DeviceRepository, entity::columns as device_cols},
        workout::{WorkoutRepository, entity::columns as workout_cols},
    },
    utils::constants,
};

#[derive(Entity)]
#[entity("workout_device")]
#[primary_key(workout, device)]
#[index("device",(device))]
pub struct WorkoutDevice {
    pub workout: u16,
    pub device: String,
    pub pending: bool,
    /// Name the workout had when last sent to the device, if renamed since: its file there is
    /// still named after it and has to be deleted before the next push
    pub previous_name: Option<String>,
}

impl WorkoutDevice {
    pub fn new(workout: u16, device: &str, pending: bool) -> Self {
        Self {
            workout,
            device: device.to_string(),
            pending,
            previous_name: None,
        }
    }

    /// Records in `workout_device` whether each of `workouts` still has to be uploaded to Connect.
    /// The upserts are idempotent, so a partial run (several statements for a huge list) is harmless.
    pub async fn set_connect_pending(
        conn: &rusqlite_orm::rusqlite::Connection,
        workouts: &[u16],
        pending: bool,
    ) -> rusqlite_orm::errors::Result<()> {
        let mut insert = WorkoutDeviceRepository::insert().or_replace();
        let mut entries: Vec<WorkoutDevice> = workouts
            .iter()
            .map(|workout| WorkoutDevice::new(*workout, constants::CONNECT_DEVICE_SERIAL, pending))
            .collect();
        for entry in &mut entries {
            insert = insert.item(entry);
        }
        insert.execute_in(conn).await?;
        Ok(())
    }

    /// Queues `workouts` again for every device that has them, Connect aside. Their Connect id and
    /// creation time key their files on devices, so a file written before those changed is stale.
    pub async fn requeue_on_devices(
        conn: &rusqlite_orm::rusqlite::Connection,
        workouts: &[u16],
    ) -> rusqlite_orm::errors::Result<()> {
        if workouts.is_empty() {
            return Ok(());
        }
        WorkoutDeviceRepository::update()
            .set(entity::columns::PENDING, true.into())
            .where_(Where::And(vec![
                Where::In(
                    entity::columns::WORKOUT,
                    workouts.iter().map(|w| (*w).into()).collect(),
                ),
                Where::NotEq(
                    entity::columns::DEVICE,
                    constants::CONNECT_DEVICE_SERIAL.into(),
                ),
            ]))
            .execute_in(conn)
            .await?;
        Ok(())
    }

    /// Devices linked to the Connect account where `workout` is still pending.
    pub async fn get_pending_connect_devices(
        conn: &rusqlite_orm::rusqlite::Connection,
        workout: u16,
    ) -> rusqlite_orm::errors::Result<Vec<String>> {
        Ok(WorkoutDeviceRepository::select()
            .columns(&[entity::columns::DEVICE])
            .where_(Where::And(vec![
                Where::Eq(entity::columns::WORKOUT, workout.into()),
                Where::Eq(entity::columns::PENDING, true.into()),
                Where::InSub(
                    entity::columns::DEVICE,
                    DeviceRepository::select()
                        .distinct(&[device_cols::SERIAL])
                        .where_(Where::Eq(device_cols::CONNECT, true.into()))
                        .to_subquery(),
                ),
            ]))
            .fetch_in(conn)
            .await?
            .iter()
            .map(|r| r.get_text(entity::columns::DEVICE.as_ref()))
            .collect())
    }

    /// Records that the device has the workout, keeping the entry if it was already there.
    pub async fn insert_or_ignore(
        conn: &Connection,
        workout: u16,
        device: &str,
    ) -> rusqlite_orm::errors::Result<usize> {
        WorkoutDeviceRepository::insert()
            .or_ignore()
            .item(&mut WorkoutDevice::new(workout, device, false))
            .execute_in(conn)
            .await
    }

    /// Records the workout for the device, replacing the `pending` flag of the entry if any. The
    /// previous name of an existing entry is kept, as its file is still on the device.
    pub async fn insert_or_replace(
        conn: &Connection,
        workout: u16,
        device: &str,
        pending: bool,
    ) -> rusqlite_orm::errors::Result<usize> {
        let updated = WorkoutDeviceRepository::update()
            .set(entity::columns::PENDING, pending.into())
            .where_(by_key(workout, device))
            .execute_in(conn)
            .await?;
        if updated > 0 {
            return Ok(updated);
        }
        WorkoutDeviceRepository::insert()
            .item(&mut WorkoutDevice::new(workout, device, pending))
            .execute_in(conn)
            .await
    }

    /// Records `old_name` as the name the renamed `workout` still has on every device holding it,
    /// Connect aside. A name already recorded is kept: it is the one of the file on the device.
    pub async fn set_previous_name(
        conn: &Connection,
        workout: u16,
        old_name: &str,
    ) -> rusqlite_orm::errors::Result<()> {
        WorkoutDeviceRepository::update()
            .set(entity::columns::PREVIOUS_NAME, old_name.into())
            .where_(Where::And(vec![
                Where::Eq(entity::columns::WORKOUT, workout.into()),
                Where::NotEq(
                    entity::columns::DEVICE,
                    constants::CONNECT_DEVICE_SERIAL.into(),
                ),
                Where::Null(entity::columns::PREVIOUS_NAME),
            ]))
            .execute_in(conn)
            .await?;
        Ok(())
    }

    /// Name the workout still has on the device `serial`, `None` if it wasn't renamed since sent.
    pub async fn get_previous_name(
        conn: &Connection,
        workout: u16,
        serial: &str,
    ) -> rusqlite_orm::errors::Result<Option<String>> {
        Ok(
            WorkoutDeviceRepository::select_by_id_in(conn, workout, serial)
                .await?
                .and_then(|entry| entry.previous_name),
        )
    }

    /// Forgets the previous name of the workout on the device `serial`, once its file is gone.
    pub async fn clear_previous_name(
        conn: &Connection,
        workout: u16,
        serial: &str,
    ) -> rusqlite_orm::errors::Result<()> {
        WorkoutDeviceRepository::update()
            .set(
                entity::columns::PREVIOUS_NAME,
                Option::<String>::None.into(),
            )
            .where_(by_key(workout, serial))
            .execute_in(conn)
            .await?;
        Ok(())
    }

    /// Workouts queued for the device `serial`, as `(id, name)` by name. With `enabled`, only the
    /// enabled ones (to upload) or the disabled ones (to delete from it).
    pub async fn get_pending(
        conn: &Connection,
        serial: &str,
        enabled: Option<bool>,
    ) -> rusqlite_orm::errors::Result<Vec<(u16, String)>> {
        let mut conditions = vec![Where::InSub(
            workout_cols::ID,
            WorkoutDeviceRepository::select()
                .columns(&[entity::columns::WORKOUT])
                .where_(Where::And(vec![
                    Where::Eq(entity::columns::DEVICE, serial.into()),
                    Where::Eq(entity::columns::PENDING, true.into()),
                ]))
                .to_subquery(),
        )];
        if let Some(enabled) = enabled {
            conditions.push(Where::Eq(workout_cols::ENABLED, enabled.into()));
        }
        Ok(WorkoutRepository::select()
            .columns(&[workout_cols::ID, workout_cols::NAME])
            .where_(Where::And(conditions))
            .order_by(OrderBy::Asc(workout_cols::NAME))
            .fetch_in(conn)
            .await?
            .iter()
            .map(|r| {
                (
                    r.get_u16(workout_cols::ID.as_ref()),
                    r.get_text(workout_cols::NAME.as_ref()),
                )
            })
            .collect())
    }

    /// Ids of the workouts already sent to the device `serial`, those with an entry not pending.
    pub async fn get_sent(
        conn: &Connection,
        serial: &str,
    ) -> rusqlite_orm::errors::Result<HashSet<u16>> {
        WorkoutDeviceRepository::select()
            .columns(&[entity::columns::WORKOUT])
            .where_(Where::And(vec![
                Where::Eq(entity::columns::DEVICE, serial.into()),
                Where::Eq(entity::columns::PENDING, false.into()),
            ]))
            .fetch_in(conn)
            .await
            .map(|rows| {
                rows.iter()
                    .map(|r| r.get_u16(entity::columns::WORKOUT.as_ref()))
                    .collect()
            })
    }

    /// Takes the workout off the queue of the device.
    pub async fn clear_pending(
        conn: &Connection,
        workout: u16,
        device: &str,
    ) -> rusqlite_orm::errors::Result<usize> {
        WorkoutDeviceRepository::update()
            .set(entity::columns::PENDING, false.into())
            .where_(by_key(workout, device))
            .execute_in(conn)
            .await
    }
}

/// Condition matching the entry of `workout` on `device`.
fn by_key(workout: u16, device: &str) -> Where<WorkoutDevice> {
    Where::And(vec![
        Where::Eq(entity::columns::WORKOUT, workout.into()),
        Where::Eq(entity::columns::DEVICE, device.into()),
    ])
}

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
    pub workout: String,
    pub device: String,
    pub pending: bool,
}

impl WorkoutDevice {
    pub fn new(workout: &str, device: &str, pending: bool) -> Self {
        Self {
            workout: workout.to_string(),
            device: device.to_string(),
            pending,
        }
    }

    /// Records in `workout_device` whether each of `workouts` still has to be uploaded to Connect.
    /// The upserts are idempotent, so a partial run (several statements for a huge list) is harmless.
    pub async fn set_connect_pending(
        conn: &rusqlite_orm::rusqlite::Connection,
        workouts: &[String],
        pending: bool,
    ) -> rusqlite_orm::errors::Result<()> {
        let mut insert = WorkoutDeviceRepository::insert().or_replace();
        let mut entries: Vec<WorkoutDevice> = workouts
            .iter()
            .map(|workout| WorkoutDevice::new(workout, constants::CONNECT_DEVICE_SERIAL, pending))
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
        workouts: &[String],
    ) -> rusqlite_orm::errors::Result<()> {
        if workouts.is_empty() {
            return Ok(());
        }
        WorkoutDeviceRepository::update()
            .set(entity::columns::PENDING, true.into())
            .where_(Where::And(vec![
                Where::In(
                    entity::columns::WORKOUT,
                    workouts.iter().map(|w| w.clone().into()).collect(),
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
        workout: &str,
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
        workout: &str,
        device: &str,
    ) -> rusqlite_orm::errors::Result<usize> {
        WorkoutDeviceRepository::insert()
            .or_ignore()
            .item(&mut WorkoutDevice::new(workout, device, false))
            .execute_in(conn)
            .await
    }

    /// Records the workout for the device, replacing the entry (and its `pending` flag) if any.
    pub async fn insert_or_replace(
        conn: &Connection,
        workout: &str,
        device: &str,
        pending: bool,
    ) -> rusqlite_orm::errors::Result<usize> {
        WorkoutDeviceRepository::insert()
            .or_replace()
            .item(&mut WorkoutDevice::new(workout, device, pending))
            .execute_in(conn)
            .await
    }

    /// Workouts queued for the device `serial`, by name. With `enabled`, only the enabled ones (to
    /// upload) or the disabled ones (to delete from it).
    pub async fn get_pending(
        conn: &Connection,
        serial: &str,
        enabled: Option<bool>,
    ) -> rusqlite_orm::errors::Result<Vec<String>> {
        let mut conditions = vec![
            Where::Eq(entity::columns::DEVICE, serial.into()),
            Where::Eq(entity::columns::PENDING, true.into()),
        ];
        if let Some(enabled) = enabled {
            conditions.push(Where::InSub(
                entity::columns::WORKOUT,
                WorkoutRepository::select()
                    .distinct(&[workout_cols::NAME])
                    .where_(Where::Eq(workout_cols::ENABLED, enabled.into()))
                    .to_subquery(),
            ));
        }
        WorkoutDeviceRepository::select()
            .columns(&[entity::columns::WORKOUT])
            .where_(Where::And(conditions))
            .order_by(OrderBy::Asc(entity::columns::WORKOUT))
            .fetch_in(conn)
            .await
            .map(|rows| {
                rows.iter()
                    .map(|r| r.get_text(entity::columns::WORKOUT.as_ref()))
                    .collect()
            })
    }

    /// Names of the workouts already sent to the device `serial`, those with an entry not pending.
    pub async fn get_sent(
        conn: &Connection,
        serial: &str,
    ) -> rusqlite_orm::errors::Result<HashSet<String>> {
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
                    .map(|r| r.get_text(entity::columns::WORKOUT.as_ref()))
                    .collect()
            })
    }

    /// Takes the workout off the queue of the device.
    pub async fn clear_pending(
        conn: &Connection,
        workout: &str,
        device: &str,
    ) -> rusqlite_orm::errors::Result<usize> {
        WorkoutDeviceRepository::update()
            .set(entity::columns::PENDING, false.into())
            .where_(Where::And(vec![
                Where::Eq(entity::columns::WORKOUT, workout.into()),
                Where::Eq(entity::columns::DEVICE, device.into()),
            ]))
            .execute_in(conn)
            .await
    }
}

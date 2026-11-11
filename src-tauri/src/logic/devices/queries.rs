use std::collections::{HashMap, HashSet};

use rusqlite_orm::{
    dao::Repository,
    database::DatabasePool,
    types::{order_by::OrderBy, where_clause::Where},
};

use crate::dao::{
    device::{self, Device, DeviceRepository},
    exercise::{self, ExerciseRepository},
    session::{self, SessionRepository},
    workout::{self, Workout, WorkoutRepository},
    workout_device::{self, WorkoutDevice, WorkoutDeviceRepository},
    workout_step::{self, WorkoutStepRepository},
};

/// Devices linked to the Connect account where `workout` is still pending.
pub(crate) fn pending_connect_devices(
    conn: &rusqlite_orm::rusqlite::Connection,
    workout: &str,
) -> rusqlite_orm::errors::Result<Vec<String>> {
    Ok(WorkoutDeviceRepository::select()
        .columns(&[workout_device::entity::columns::DEVICE])
        .where_(Where::And(vec![
            Where::Eq(workout_device::entity::columns::WORKOUT, workout.into()),
            Where::Eq(workout_device::entity::columns::PENDING, true.into()),
            Where::InSub(
                workout_device::entity::columns::DEVICE,
                DeviceRepository::select()
                    .distinct(&[device::entity::columns::SERIAL])
                    .where_(Where::Eq(device::entity::columns::CONNECT, true.into()))
                    .to_subquery(),
            ),
        ]))
        .fetch_in(conn)?
        .iter()
        .map(|r| r.get_text(workout_device::entity::columns::DEVICE.as_ref()))
        .collect())
}

/// Registers a device plugged by USB, or flags a known one as reachable by USB.
pub(super) fn enrol_usb_device(
    tx: &rusqlite_orm::rusqlite::Transaction,
    device: &mut Device,
) -> rusqlite_orm::errors::Result<()> {
    let _: () = if let Some(device) = DeviceRepository::select_by_id_in(tx, &device.serial)? {
        if !device.usb {
            DeviceRepository::update()
                .set(device::entity::columns::USB, true.into())
                .where_(Where::Eq(
                    device::entity::columns::SERIAL,
                    device.serial.clone().into(),
                ))
                .execute_in(tx)
                .map(|_| ())?
        }
    } else {
        DeviceRepository::insert()
            .item(device)
            .execute_in(tx)
            .map(|_| ())?
    };
    Ok(())
}

/// Stores the notes and steps of a workout downloaded from a device.
pub(super) fn store_workout_steps(
    tx: &rusqlite_orm::rusqlite::Transaction,
    workout: &mut Workout,
) -> rusqlite_orm::errors::Result<()> {
    WorkoutRepository::update()
        .set(
            workout::entity::columns::NOTES,
            workout.notes.clone().into(),
        )
        .where_(Where::Eq(
            workout::entity::columns::NAME,
            workout.name.clone().into(),
        ))
        .execute_in(tx)?;

    let mut insert = WorkoutStepRepository::insert();
    for step in &mut workout.steps {
        insert = insert.item(step);
    }

    insert.execute_in(tx)?;

    Ok(())
}

/// The workout with its steps, those without notes of their own inheriting the exercise notes.
pub(super) fn workout_for_device(
    conn: &rusqlite_orm::rusqlite::Connection,
    name: &str,
) -> rusqlite_orm::errors::Result<Workout> {
    let mut wk = WorkoutRepository::select_by_id_in(conn, name)?.unwrap();
    wk.fetch_steps_relationship_in(conn)?;

    // Exercise steps without their own notes inherit the exercise notes
    let pending: Vec<(usize, (u16, u16))> = wk
        .steps
        .iter()
        .enumerate()
        .filter_map(|(idx, step)| {
            if step.notes.as_deref().is_none_or(str::is_empty)
                && let Some(id) = step.ex_id
                && let Some(cat) = step.ex_cat
            {
                Some((idx, (cat, id)))
            } else {
                None
            }
        })
        .collect();

    if !pending.is_empty() {
        let pks: HashSet<(u16, u16)> = pending.iter().map(|(_, pk)| *pk).collect();
        let notes = ExerciseRepository::select()
            .where_(Where::InMultiple(
                vec![
                    exercise::entity::columns::CATEGORY,
                    exercise::entity::columns::ID,
                ],
                pks.iter()
                    .map(|(cat, id)| vec![(*cat).into(), (*id).into()])
                    .collect(),
            ))
            .fetch_in(conn)?
            .into_iter()
            .filter_map(|e| Some(((e.category, e.id), e.notes?)))
            .collect::<HashMap<_, _>>();

        for (idx, pk) in pending {
            if let Some(note) = notes.get(&pk) {
                wk.steps[idx].notes = Some(note.clone());
            }
        }
    }

    Ok(wk)
}

/// Records that the device has the workout, keeping the entry if it was already there.
pub(super) async fn add_workout_device(
    database: &DatabasePool,
    entry: &mut WorkoutDevice,
) -> rusqlite_orm::errors::Result<usize> {
    WorkoutDeviceRepository::insert()
        .or_ignore()
        .item(entry)
        .execute(database)
        .await
}

/// Records the workout for the device, replacing the entry (and its `pending` flag) if any.
pub(crate) async fn queue_workout_device(
    database: &DatabasePool,
    entry: &mut WorkoutDevice,
) -> rusqlite_orm::errors::Result<usize> {
    WorkoutDeviceRepository::insert()
        .or_replace()
        .item(entry)
        .execute(database)
        .await
}

/// The registered device with that serial number.
///
/// # Panics
/// If the device isn't registered.
pub(super) async fn device_by_serial(
    database: &DatabasePool,
    serial: &str,
) -> rusqlite_orm::errors::Result<Device> {
    DeviceRepository::select_by_id(database, serial)
        .await
        .map(Option::unwrap)
}

/// Every registered device, the Connect pseudo-device included.
pub(super) async fn all_devices(
    database: &DatabasePool,
) -> rusqlite_orm::errors::Result<Vec<Device>> {
    DeviceRepository::select().fetch(database).await
}

/// Workouts queued for the device `serial`, by name: the enabled ones to upload, or the disabled
/// ones to delete from it.
pub(super) async fn pending_queue(
    db: &DatabasePool,
    serial: &str,
    enabled: bool,
) -> rusqlite_orm::errors::Result<Vec<String>> {
    WorkoutDeviceRepository::select()
        .columns(&[workout_device::entity::columns::WORKOUT])
        .where_(Where::And(vec![
            Where::InSub(
                workout_device::entity::columns::WORKOUT,
                WorkoutRepository::select()
                    .distinct(&[workout::entity::columns::NAME])
                    .where_(Where::Eq(workout::entity::columns::ENABLED, enabled.into()))
                    .to_subquery(),
            ),
            Where::Eq(workout_device::entity::columns::DEVICE, serial.into()),
            Where::Eq(workout_device::entity::columns::PENDING, true.into()),
        ]))
        .order_by(OrderBy::Asc(workout_device::entity::columns::WORKOUT))
        .fetch(db)
        .await
        .map(|rows| {
            rows.iter()
                .map(|r| r.get_text(workout_device::entity::columns::WORKOUT.as_ref()))
                .collect()
        })
}

/// Takes the workout off the queue of the device.
pub(super) async fn clear_pending(
    db: &DatabasePool,
    workout: &str,
    device: &str,
) -> rusqlite_orm::errors::Result<usize> {
    WorkoutDeviceRepository::update()
        .set(workout_device::entity::columns::PENDING, false.into())
        .where_(Where::And(vec![
            Where::Eq(workout_device::entity::columns::WORKOUT, workout.into()),
            Where::Eq(workout_device::entity::columns::DEVICE, device.into()),
        ]))
        .execute(db)
        .await
}

/// Workouts recorded in sessions of the device `serial` that have no steps stored yet.
pub(super) async fn workouts_without_steps(
    db: &DatabasePool,
    serial: &str,
) -> rusqlite_orm::errors::Result<Vec<String>> {
    Ok(WorkoutRepository::select()
        .columns(&[workout::entity::columns::NAME])
        .where_(Where::And(vec![
            Where::NotInSub(
                workout::entity::columns::NAME,
                WorkoutStepRepository::select()
                    .distinct(&[workout_step::entity::columns::WORKOUT])
                    .to_subquery(),
            ),
            Where::InSub(
                workout::entity::columns::NAME,
                SessionRepository::select()
                    .distinct(&[session::entity::columns::WORKOUT])
                    .where_(Where::Eq(session::entity::columns::DEVICE, serial.into()))
                    .to_subquery(),
            ),
        ]))
        .fetch(db)
        .await?
        .iter()
        .map(|r| r.get_text(workout::entity::columns::NAME.as_ref()))
        .collect())
}

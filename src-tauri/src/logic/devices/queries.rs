use std::collections::{HashMap, HashSet};

use rusqlite_orm::{
    dao::Repository,
    database::DatabasePool,
    types::{order_by::OrderBy, where_clause::Where},
};

use crate::dao::{
    device::{Device, DeviceRepository, entity::columns as device_cols},
    exercise::{ExerciseRepository, entity::columns as exercise_cols},
    session::{SessionRepository, entity::columns as session_cols},
    workout::{Workout, WorkoutRepository, entity::columns as workout_cols},
    workout_device::{
        WorkoutDevice, WorkoutDeviceRepository, entity::columns as workout_device_cols,
    },
    workout_step::{WorkoutStepRepository, entity::columns as workout_step_cols},
};

/// Devices linked to the Connect account where `workout` is still pending.
pub(crate) fn pending_connect_devices(
    conn: &rusqlite_orm::rusqlite::Connection,
    workout: &str,
) -> rusqlite_orm::errors::Result<Vec<String>> {
    Ok(WorkoutDeviceRepository::select()
        .columns(&[workout_device_cols::DEVICE])
        .where_(Where::And(vec![
            Where::Eq(workout_device_cols::WORKOUT, workout.into()),
            Where::Eq(workout_device_cols::PENDING, true.into()),
            Where::InSub(
                workout_device_cols::DEVICE,
                DeviceRepository::select()
                    .distinct(&[device_cols::SERIAL])
                    .where_(Where::Eq(device_cols::CONNECT, true.into()))
                    .to_subquery(),
            ),
        ]))
        .fetch_in(conn)?
        .iter()
        .map(|r| r.get_text(workout_device_cols::DEVICE.as_ref()))
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
                .set(device_cols::USB, true.into())
                .where_(Where::Eq(device_cols::SERIAL, device.serial.clone().into()))
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
        .set(workout_cols::NOTES, workout.notes.clone().into())
        .where_(Where::Eq(workout_cols::NAME, workout.name.clone().into()))
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
                vec![exercise_cols::CATEGORY, exercise_cols::ID],
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
        .columns(&[workout_device_cols::WORKOUT])
        .where_(Where::And(vec![
            Where::InSub(
                workout_device_cols::WORKOUT,
                WorkoutRepository::select()
                    .distinct(&[workout_cols::NAME])
                    .where_(Where::Eq(workout_cols::ENABLED, enabled.into()))
                    .to_subquery(),
            ),
            Where::Eq(workout_device_cols::DEVICE, serial.into()),
            Where::Eq(workout_device_cols::PENDING, true.into()),
        ]))
        .order_by(OrderBy::Asc(workout_device_cols::WORKOUT))
        .fetch(db)
        .await
        .map(|rows| {
            rows.iter()
                .map(|r| r.get_text(workout_device_cols::WORKOUT.as_ref()))
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
        .set(workout_device_cols::PENDING, false.into())
        .where_(Where::And(vec![
            Where::Eq(workout_device_cols::WORKOUT, workout.into()),
            Where::Eq(workout_device_cols::DEVICE, device.into()),
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
        .columns(&[workout_cols::NAME])
        .where_(Where::And(vec![
            Where::NotInSub(
                workout_cols::NAME,
                WorkoutStepRepository::select()
                    .distinct(&[workout_step_cols::WORKOUT])
                    .to_subquery(),
            ),
            Where::InSub(
                workout_cols::NAME,
                SessionRepository::select()
                    .distinct(&[session_cols::WORKOUT])
                    .where_(Where::Eq(session_cols::DEVICE, serial.into()))
                    .to_subquery(),
            ),
        ]))
        .fetch(db)
        .await?
        .iter()
        .map(|r| r.get_text(workout_cols::NAME.as_ref()))
        .collect())
}

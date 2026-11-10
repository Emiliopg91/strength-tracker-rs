use std::collections::{BTreeSet, HashMap};

use rusqlite_orm::{
    dao::Repository,
    database::DatabasePool,
    types::{order_by::OrderBy, where_clause::Where},
};

use crate::{
    dao::{
        body_metric::{self, BodyMetric, BodyMetricRepository},
        device::{self, Device, DeviceRepository},
        exercise::{self, Exercise, ExerciseRepository},
        exercise_category::{self, ExerciseCategory, ExerciseCategoryRepository},
        session::{self, SessionRepository},
        workout::{Workout, WorkoutRepository},
        workout_device::{self, WorkoutDevice, WorkoutDeviceRepository},
        workout_step::WorkoutStep,
    },
    dto::{connect::ConnectUpdateSets, devices::DeviceListItem, sessions::SessionStepDto},
    utils::constants,
};

/// Workouts that still have to be uploaded to Connect.
pub(crate) fn pending_connect_workouts(
    conn: &rusqlite_orm::rusqlite::Connection,
) -> rusqlite_orm::errors::Result<Vec<String>> {
    Ok(WorkoutDeviceRepository::select()
        .where_(Where::And(vec![
            Where::Eq(
                workout_device::entity::columns::DEVICE,
                constants::CONNECT_DEVICE_SERIAL.into(),
            ),
            Where::Eq(workout_device::entity::columns::PENDING, true.into()),
        ]))
        .order_by(OrderBy::Asc(workout_device::entity::columns::WORKOUT))
        .fetch_in(conn)?
        .into_iter()
        .map(|e| e.workout)
        .collect())
}

/// Records in `workout_device` whether each of `workouts` still has to be uploaded to Connect.
/// The upserts are idempotent, so a partial run (several statements for a huge list) is harmless.
pub(crate) fn set_connect_pending(
    conn: &rusqlite_orm::rusqlite::Connection,
    workouts: &[String],
    pending: bool,
) -> rusqlite_orm::errors::Result<()> {
    let mut insert = WorkoutDeviceRepository::insert().or_replace();
    let mut entries: Vec<WorkoutDevice> = workouts
        .iter()
        .map(|workout| WorkoutDevice {
            workout: workout.clone(),
            device: constants::CONNECT_DEVICE_SERIAL.to_string(),
            pending,
        })
        .collect();
    for entry in &mut entries {
        insert = insert.item(entry);
    }
    insert.execute_in(conn)?;
    Ok(())
}

/// Fills the exercise, with its category, of every exercise step in two queries, instead of
/// two per step as the relationship fetchers would.
pub(crate) fn fetch_step_exercises(
    conn: &rusqlite_orm::rusqlite::Connection,
    steps: &mut [WorkoutStep],
) -> rusqlite_orm::errors::Result<()> {
    let keys: BTreeSet<(u16, u16)> = steps.iter().filter_map(|s| s.ex_cat.zip(s.ex_id)).collect();
    if keys.is_empty() {
        return Ok(());
    }
    let exercises = fetch_exercises_with_category(conn, &keys)?;

    for step in steps {
        step.exercise = step
            .ex_cat
            .zip(step.ex_id)
            .and_then(|key| exercises.get(&key).cloned());
    }
    Ok(())
}

/// Loads the exercises with the given `(category, id)` keys, with their category filled, in two queries.
pub(super) fn fetch_exercises_with_category(
    conn: &rusqlite_orm::rusqlite::Connection,
    keys: &BTreeSet<(u16, u16)>,
) -> rusqlite_orm::errors::Result<HashMap<(u16, u16), Exercise>> {
    let mut exercises: HashMap<(u16, u16), Exercise> = ExerciseRepository::select()
        .where_(Where::InMultiple(
            vec![
                exercise::entity::columns::CATEGORY,
                exercise::entity::columns::ID,
            ],
            keys.iter()
                .map(|(cat, id)| vec![(*cat).into(), (*id).into()])
                .collect(),
        ))
        .fetch_in(conn)?
        .into_iter()
        .map(|e| ((e.category, e.id), e))
        .collect();

    let category_ids: BTreeSet<u16> = keys.iter().map(|(cat, _)| *cat).collect();
    let categories: HashMap<u16, ExerciseCategory> = ExerciseCategoryRepository::select()
        .where_(Where::In(
            exercise_category::entity::columns::ID,
            category_ids.iter().map(|id| (*id).into()).collect(),
        ))
        .fetch_in(conn)?
        .into_iter()
        .map(|c| (c.id, c))
        .collect();
    for exercise in exercises.values_mut() {
        exercise.exercise_category = categories.get(&exercise.category).cloned();
    }
    Ok(exercises)
}

/// The workout with its steps in order and their exercises, `None` if it doesn't exist.
pub(super) fn workout_with_exercises(
    conn: &rusqlite_orm::rusqlite::Connection,
    id: &str,
) -> rusqlite_orm::errors::Result<Option<Workout>> {
    let Some(mut workout) = WorkoutRepository::select_by_id_in(conn, id)? else {
        return Ok(None);
    };
    workout.fetch_steps_relationship_in(conn)?;
    workout.steps.sort_by_key(|s| s.idx);
    fetch_step_exercises(conn, &mut workout.steps)?;
    Ok(Some(workout))
}

/// Connect activity id of the session and the edited `sets` named as Connect expects, `None` if
/// the session isn't linked to an activity.
pub(super) fn session_sets_for_connect(
    conn: &rusqlite_orm::rusqlite::Connection,
    timestamp: u32,
    sets: &[SessionStepDto],
) -> rusqlite_orm::errors::Result<Option<(String, Vec<ConnectUpdateSets>)>> {
    let Some(connect_id) = SessionRepository::select()
        .columns(&[session::entity::columns::CONNECT_ID])
        .where_(Where::Eq(session::entity::columns::DATE, timestamp.into()))
        .fetch_in(conn)?
        .first()
        .and_then(|row| row.get_text_nullable(session::entity::columns::CONNECT_ID.as_ref()))
    else {
        return Ok(None);
    };

    let keys: BTreeSet<(u16, u16)> = sets.iter().filter_map(|s| s.ex_cat.zip(s.ex_id)).collect();
    let names: HashMap<(u16, u16), (String, String)> = fetch_exercises_with_category(conn, &keys)?
        .into_iter()
        .filter_map(|(key, exercise)| {
            Some((key, (exercise.exercise_category?.name, exercise.name)))
        })
        .collect();

    let updates: Vec<ConnectUpdateSets> = sets
        .iter()
        .filter_map(|set| {
            let (exercise_category, exercise_id) = names.get(&set.ex_cat.zip(set.ex_id)?)?.clone();
            Some(ConnectUpdateSets {
                idx: set.idx as u16,
                reps: set.reps,
                weight: set.weight,
                exercise_category,
                exercise_id,
            })
        })
        .collect();
    Ok(Some((connect_id, updates)))
}

/// Enrolls the workout-capable Connect devices in `listed`, refreshes the Connect-owned fields of
/// known ones and clears the `connect` flag of those no longer listed. Returns the added or changed
/// devices, to be announced.
pub(super) fn sync_connect_devices(
    tx: &rusqlite_orm::rusqlite::Transaction,
    listed: &[Device],
) -> rusqlite_orm::errors::Result<Vec<DeviceListItem>> {
    let mut discovered = Vec::new();
    for mut dao in listed.iter().cloned() {
        match DeviceRepository::select_by_id_in(tx, &dao.serial)? {
            None => {
                DeviceRepository::insert().item(&mut dao).execute_in(tx)?;
            }
            Some(db)
                if !db.connect
                    || db.model != dao.model
                    || db.strength_workout != dao.strength_workout =>
            {
                DeviceRepository::update()
                    .set(device::entity::columns::CONNECT, true.into())
                    .set(device::entity::columns::MODEL, dao.model.clone().into())
                    .set(
                        device::entity::columns::STRENGTH_WORKOUT,
                        dao.strength_workout.into(),
                    )
                    .where_(Where::Eq(
                        device::entity::columns::SERIAL,
                        dao.serial.clone().into(),
                    ))
                    .execute_in(tx)?;
            }
            Some(_) => {}
        }

        discovered.push(DeviceListItem::from(&dao));
    }

    let removed: Vec<Device> = DeviceRepository::select()
        .where_(Where::Eq(device::entity::columns::CONNECT, true.into()))
        .fetch_in(tx)?
        .into_iter()
        .filter(|db| !listed.iter().any(|d| d.serial == db.serial))
        .collect();

    if !removed.is_empty() {
        DeviceRepository::update()
            .set(device::entity::columns::CONNECT, false.into())
            .where_(Where::In(
                device::entity::columns::SERIAL,
                removed.iter().map(|d| d.serial.clone().into()).collect(),
            ))
            .execute_in(tx)?;
    }

    Ok(discovered)
}

/// The Connect pseudo-device and the dates of every stored session, to import only new activities.
pub(super) fn connect_import_state(
    database: &DatabasePool,
) -> rusqlite_orm::errors::Result<(Device, BTreeSet<u32>)> {
    let device = DeviceRepository::select()
        .where_(Where::Eq(
            device::entity::columns::SERIAL,
            constants::CONNECT_DEVICE_SERIAL.into(),
        ))
        .fetch_one(database)?
        .unwrap();

    let existing: BTreeSet<u32> = SessionRepository::select()
        .columns(&[session::entity::columns::DATE])
        .fetch(database)?
        .iter()
        .map(|row| row.get_u32(session::entity::columns::DATE.as_ref()))
        .collect();

    Ok((device, existing))
}

/// Dates of the stored body measurements, newest first.
pub(super) fn body_metric_dates(database: &DatabasePool) -> rusqlite_orm::errors::Result<Vec<u32>> {
    Ok(BodyMetricRepository::select()
        .columns(&[body_metric::entity::columns::DATE])
        .order_by(OrderBy::Desc(body_metric::entity::columns::DATE))
        .fetch(database)?
        .iter()
        .map(|row| row.get_u32(session::entity::columns::DATE.as_ref()))
        .collect())
}

/// Stores the `entries` not logged yet (by date); returns how many were added.
pub(super) fn insert_new_body_metrics(
    database: &DatabasePool,
    entries: &mut [BodyMetric],
    existing: &[u32],
) -> rusqlite_orm::errors::Result<usize> {
    // Runs in a transaction only when several entries are inserted
    let mut insert = BodyMetricRepository::insert().or_replace();
    for entry in entries {
        if !existing.contains(&entry.date) {
            insert = insert.item(entry);
        }
    }
    insert.execute(database)
}

/// Connect id of the workout, `None` if it doesn't exist or hasn't been uploaded.
pub(super) fn workout_connect_id(
    database: &DatabasePool,
    name: &str,
) -> rusqlite_orm::errors::Result<Option<String>> {
    Ok(WorkoutRepository::select_by_id(database, name)?.and_then(|w| w.connect_id))
}

use std::collections::{HashMap, HashSet};

use rusqlite_orm::{
    Entity,
    dao::Repository,
    rusqlite::{Connection, Transaction},
    types::where_clause::Where,
};
use serde::{Deserialize, Serialize};

use crate::dao::{
    exercise::Exercise,
    session::{SessionRepository, entity::columns as session_cols},
    workout_device::{
        WorkoutDevice, WorkoutDeviceRepository, entity::columns as workout_device_cols,
    },
    workout_step::{
        self, WorkoutStep, WorkoutStepRepository, entity::columns as workout_step_cols,
    },
};

#[derive(Entity, Clone, Serialize, Deserialize)]
#[primary_key(id)]
#[unique("name", (name))]
pub struct Workout {
    /// Assigned by the database on insert; 0 until then
    #[autoincrement]
    #[serde(default)]
    pub id: u16,
    pub name: String,
    pub enabled: bool,
    pub notes: Option<String>,
    pub connect_id: Option<String>,
    /// Creation time of the workout in Connect, UTC timestamp in seconds
    #[serde(default)]
    pub connect_created_time: Option<u32>,
    pub progression: bool,

    #[relationship((id, workout_step::entity::columns::WORKOUT))]
    pub steps: Vec<WorkoutStep>,
}

impl Workout {
    pub fn get_workout_file_name(&self) -> String {
        Self::file_name_for(&self.name)
    }

    /// Device file name for the workout called `name`, truncated to 31 bytes like the device expects.
    pub fn file_name_for(name: &str) -> String {
        let name = &name[..name.floor_char_boundary(31)];
        format!("{}_workout.fit", name.replace(' ', "_"))
    }

    /// The workout with its steps in order, `None` if it doesn't exist.
    pub async fn get_with_steps(
        conn: &Connection,
        id: u16,
    ) -> rusqlite_orm::errors::Result<Option<Workout>> {
        let Some(mut workout) = WorkoutRepository::select_by_id_in(conn, id).await? else {
            return Ok(None);
        };
        workout.fetch_steps_relationship_in(conn).await?;
        workout.steps.sort_by_key(|s| s.idx);
        Ok(Some(workout))
    }

    /// The workout with its steps in order and their exercises, `None` if it doesn't exist.
    pub async fn get_with_exercises(
        conn: &Connection,
        id: u16,
    ) -> rusqlite_orm::errors::Result<Option<Workout>> {
        let Some(mut workout) = Workout::get_with_steps(conn, id).await? else {
            return Ok(None);
        };
        WorkoutStep::fill_exercises(conn, &mut workout.steps).await?;
        Ok(Some(workout))
    }

    /// The workout with its steps in order, those without notes of their own inheriting the
    /// exercise notes, `None` if it doesn't exist.
    pub async fn get_for_device(
        conn: &Connection,
        id: u16,
    ) -> rusqlite_orm::errors::Result<Option<Workout>> {
        let Some(mut wk) = Workout::get_with_steps(conn, id).await? else {
            return Ok(None);
        };

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
            let notes = Exercise::get_notes(conn, &pks).await?;
            for (idx, pk) in pending {
                if let Some(note) = notes.get(&pk) {
                    wk.steps[idx].notes = Some(note.clone());
                }
            }
        }

        Ok(Some(wk))
    }

    /// Stores the notes and steps of a workout downloaded from a device on the stored workout with
    /// its name, if any.
    pub async fn store_steps(
        tx: &Transaction<'_>,
        workout: &mut Workout,
    ) -> rusqlite_orm::errors::Result<()> {
        let Some(id) = Workout::get_id(tx, &workout.name).await? else {
            return Ok(());
        };
        workout.id = id;

        WorkoutRepository::update()
            .set(entity::columns::NOTES, workout.notes.clone().into())
            .where_(Where::Eq(entity::columns::ID, id.into()))
            .execute_in(tx)
            .await?;

        for step in &mut workout.steps {
            step.workout = id;
        }
        WorkoutStep::insert_all(tx, &mut workout.steps).await?;

        Ok(())
    }

    /// Id of the workout called `name`, `None` if there is none.
    pub async fn get_id(
        conn: &Connection,
        name: &str,
    ) -> rusqlite_orm::errors::Result<Option<u16>> {
        Ok(WorkoutRepository::select_by_name_in(conn, name)
            .await?
            .map(|workout| workout.id))
    }

    /// Workouts recorded in sessions of the device `serial` that have no steps stored yet.
    pub async fn get_without_steps(
        conn: &Connection,
        serial: &str,
    ) -> rusqlite_orm::errors::Result<Vec<String>> {
        Ok(WorkoutRepository::select()
            .columns(&[entity::columns::NAME])
            .where_(Where::And(vec![
                Where::NotInSub(
                    entity::columns::ID,
                    WorkoutStepRepository::select()
                        .distinct(&[workout_step_cols::WORKOUT])
                        .to_subquery(),
                ),
                Where::InSub(
                    entity::columns::ID,
                    SessionRepository::select()
                        .distinct(&[session_cols::WORKOUT])
                        .where_(Where::Eq(session_cols::DEVICE, serial.into()))
                        .to_subquery(),
                ),
            ]))
            .fetch_in(conn)
            .await?
            .iter()
            .map(|r| r.get_text(entity::columns::NAME.as_ref()))
            .collect())
    }

    /// Stores the Connect creation time of each `(connect id, created time)` on the workout with that
    /// Connect id, skipping the ones already up to date, and queues the changed workouts again for
    /// their devices. Returns how many workouts changed.
    pub async fn set_connect_created_times(
        tx: &Transaction<'_>,
        created_times: &[(String, u32)],
    ) -> rusqlite_orm::errors::Result<usize> {
        if created_times.is_empty() {
            return Ok(0);
        }
        let created_by_id: HashMap<&str, u32> = created_times
            .iter()
            .map(|(connect_id, created)| (connect_id.as_str(), *created))
            .collect();

        // A single query: most listed workouts are not in the database
        let linked = WorkoutRepository::select()
            .where_(Where::In(
                entity::columns::CONNECT_ID,
                created_by_id.keys().map(|id| (*id).into()).collect(),
            ))
            .fetch_in(tx)
            .await?;

        let mut changed = Vec::new();
        for mut workout in linked {
            let created = workout
                .connect_id
                .as_deref()
                .and_then(|id| created_by_id.get(id))
                .copied();
            if created.is_none() || workout.connect_created_time == created {
                continue;
            }
            workout.connect_created_time = created;
            workout.update_by_id_in(tx).await?;
            changed.push(workout.id);
        }
        WorkoutDevice::requeue_on_devices(tx, &changed).await?;
        Ok(changed.len())
    }

    /// Saves the new Connect id and creation time of the workout and queues it again for its devices.
    pub async fn set_connect_keys(&self, tx: &Transaction<'_>) -> rusqlite_orm::errors::Result<()> {
        self.update_by_id_in(tx).await?;
        WorkoutDevice::requeue_on_devices(tx, &[self.id]).await
    }

    /// Every workout, without its steps.
    pub async fn get_all(conn: &Connection) -> rusqlite_orm::errors::Result<Vec<Workout>> {
        WorkoutRepository::select().fetch_in(conn).await
    }

    /// The workout, without its steps, `None` if it doesn't exist.
    pub async fn get(conn: &Connection, id: u16) -> rusqlite_orm::errors::Result<Option<Workout>> {
        WorkoutRepository::select_by_id_in(conn, id).await
    }

    /// Enables or disables the workout and queues the change for the devices that have it.
    pub async fn set_enabled(
        tx: &Transaction<'_>,
        workout: u16,
        status: bool,
    ) -> rusqlite_orm::errors::Result<()> {
        let res = WorkoutRepository::update()
            .where_(Where::Eq(entity::columns::ID, workout.into()))
            .set(entity::columns::ENABLED, status.into())
            .execute_in(tx)
            .await
            .map(|_| ());

        if res.is_ok() {
            // The enabled flag only exists on devices, so Connect is left as it was
            let _ = WorkoutDevice::requeue_on_devices(tx, &[workout]).await;
        }

        res
    }

    /// Inserts the workout, or replaces the notes and steps of the existing one with its id when
    /// editing (its name is kept), and queues it for the devices that already have it. Returns the
    /// id of the workout.
    pub async fn store(
        tx: &Transaction<'_>,
        workout: &Workout,
        edit: bool,
    ) -> rusqlite_orm::errors::Result<u16> {
        let id = if edit
            && WorkoutRepository::select_by_id_in(tx, workout.id)
                .await?
                .is_some()
        {
            let id = workout.id;
            WorkoutRepository::update()
                .set(
                    entity::columns::NOTES,
                    (if let Some(note) = workout.notes.clone()
                        && !note.is_empty()
                    {
                        Some(note)
                    } else {
                        None
                    })
                    .into(),
                )
                .where_(Where::Eq(entity::columns::ID, id.into()))
                .execute_in(tx)
                .await?;

            WorkoutStepRepository::delete()
                .where_(Where::Eq(workout_step_cols::WORKOUT, id.into()))
                .execute_in(tx)
                .await?;

            // Re-send the edited workout to every device that already has it
            WorkoutDeviceRepository::update()
                .set(workout_device_cols::PENDING, true.into())
                .where_(Where::Eq(workout_device_cols::WORKOUT, id.into()))
                .execute_in(tx)
                .await?;
            id
        } else {
            let mut item = workout.clone();
            WorkoutRepository::insert()
                .item(&mut item)
                .execute_in(tx)
                .await?;
            item.id
        };

        let mut steps = workout.steps.clone();
        for step in &mut steps {
            step.workout = id;
            // Empty notes are stored as NULL so the exercise notes fallback applies on upload
            if step.notes.as_deref().is_some_and(str::is_empty) {
                step.notes = None;
            }
        }
        WorkoutStep::insert_all(tx, &mut steps).await?;

        Ok(id)
    }

    pub async fn workout_with_progression(
        conn: &Connection,
    ) -> rusqlite_orm::errors::Result<Vec<u16>> {
        Ok(WorkoutRepository::select()
            .columns(&[entity::columns::ID])
            .where_(Where::Eq(entity::columns::PROGRESSION, true.into()))
            .fetch_in(conn)
            .await?
            .iter()
            .map(|r| r.get_u16(entity::columns::ID.as_ref()))
            .collect())
    }
}

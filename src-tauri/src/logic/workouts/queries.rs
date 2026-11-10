use std::collections::{HashMap, HashSet};

use rusqlite_orm::{
    dao::Repository,
    types::{order_by::OrderBy, where_clause::Where},
};
use tauri_plugin_log::log::info;

use crate::{
    dao::{
        exercise::{self, ExerciseRepository},
        session::{self, SessionRepository, entity},
        session_step::{self, SessionStepRepository},
        workout::{self, Workout, WorkoutRepository},
        workout_device::{self, WorkoutDeviceRepository},
        workout_step::{self, StepType, WorkoutStepRepository},
    },
    dto::workouts::{WorkoutDetails, WorkoutListItem, WorkoutSession},
    logic::exercises::queries::fetch_exercises_muscles,
    utils::constants,
};

/// Every workout with its session stats (count, average time and load, latest date), by name.
pub fn workout_list(
    conn: &rusqlite_orm::rusqlite::Connection,
) -> rusqlite_orm::errors::Result<Vec<WorkoutListItem>> {
    let workouts: Vec<_> = WorkoutRepository::select()
        .fetch_in(conn)?
        .into_iter()
        .collect();

    let sessions = SessionRepository::select()
        .columns(&[
            entity::columns::NAME,
            entity::columns::DATE,
            entity::columns::TOTAL_ELAPSED_TIME,
            entity::columns::TRAINING_LOAD,
        ])
        .where_(Where::NotNull(session::entity::columns::WORKOUT))
        .order_by(OrderBy::Desc(entity::columns::DATE))
        .fetch_in(conn)?;

    let mut workout_stats = HashMap::new();
    workouts.iter().for_each(|w| {
        workout_stats.insert(w.name.clone(), (0, 0, None, 0_u32));
    });

    let suggested_workouts: HashSet<_> = workouts
        .iter()
        .filter_map(|w| {
            if w.enabled {
                Some(w.name.clone())
            } else {
                None
            }
        })
        .collect();

    sessions.iter().for_each(|s| {
        let entry = workout_stats
            .entry(s.get_text(entity::columns::NAME.as_ref()))
            .or_insert((0_u32, 0_u32, None, 0_u32));
        entry.0 += 1_u32;
        entry.1 += s.get_u32(entity::columns::TOTAL_ELAPSED_TIME.as_ref());
        if entry.2.is_none() {
            entry.2 = Some(s.get_u32(entity::columns::DATE.as_ref()));
        }
        entry.3 += s.get_u32(entity::columns::TRAINING_LOAD.as_ref())
    });

    let with_steps: HashSet<_> = WorkoutStepRepository::select()
        .distinct(&[workout_step::entity::columns::WORKOUT])
        .fetch_in(conn)?
        .iter()
        .map(|r| r.get_text(workout_step::entity::columns::WORKOUT.as_ref()))
        .collect();

    let mut res: Vec<_> = workout_stats
        .into_iter()
        .map(|wd| WorkoutListItem {
            has_steps: with_steps.contains(&wd.0),
            enabled: suggested_workouts.contains(&wd.0),
            name: wd.0,
            sessions: wd.1.0,
            avg_time: wd.1.1.checked_div(wd.1.0).unwrap_or(0),
            avg_load: wd.1.3.checked_div(wd.1.0).unwrap_or(0) as u16,
            latest_session: wd.1.2,
        })
        .collect();

    res.sort_by(|a, b| a.name.cmp(&b.name));

    Ok(res)
}

/// The workout with its steps, worked muscles and every session with its volume.
pub fn workout_details(
    conn: &rusqlite_orm::rusqlite::Connection,
    name: &str,
) -> rusqlite_orm::errors::Result<WorkoutDetails> {
    info!("Getting details for workout {}", name);

    let mut workout = WorkoutRepository::select_by_id_in(conn, name)?.unwrap();
    workout.fetch_steps_relationship_in(conn)?;
    workout.steps.sort_by_key(|e| e.idx);

    let sessions = SessionRepository::select_by_name_in(
        conn,
        name,
        Some(&[OrderBy::Desc(entity::columns::DATE)]),
    )?;

    let mut exercises = ExerciseRepository::select()
        .where_(Where::InMultipleSub(
            vec![
                exercise::entity::columns::CATEGORY,
                exercise::entity::columns::ID,
            ],
            WorkoutStepRepository::select()
                .distinct(&[
                    workout_step::entity::columns::EX_CAT,
                    workout_step::entity::columns::EX_ID,
                ])
                .where_(Where::Eq(
                    workout_step::entity::columns::WORKOUT,
                    workout.name.clone().into(),
                ))
                .to_subquery(),
        ))
        .fetch_in(conn)?;

    fetch_exercises_muscles(conn, &mut exercises)?;
    let mut primary_set = HashSet::new();
    let mut secondary_set = HashSet::new();
    for exercise in &exercises {
        for muscle in &exercise.muscles {
            if muscle.is_primary {
                primary_set.insert(muscle.muscle.clone());
            } else {
                secondary_set.insert(muscle.muscle.clone());
            }
        }
    }

    let secondary_muscles = secondary_set
        .into_iter()
        .filter(|m| !primary_set.contains(m))
        .collect();
    let primary_muscles = primary_set.into_iter().collect();

    let latest = sessions.first().map(|latest| latest.date);
    let mut count = 0_u32;
    let mut time = 0_u32;
    let mut volume = 0_f32;

    let series = SessionStepRepository::select()
        .columns(&[
            session_step::entity::columns::SESSION,
            session_step::entity::columns::REPS,
            session_step::entity::columns::WEIGHT,
        ])
        .where_(Where::And(vec![
            // Rest steps have neither reps nor weight
            Where::NotEq(session_step::entity::columns::KIND, StepType::Rest.into()),
            Where::InSub(
                session_step::entity::columns::SESSION,
                SessionRepository::select()
                    .columns(&[entity::columns::DATE])
                    .where_(Where::Eq(entity::columns::NAME, name.into()))
                    .to_subquery(),
            ),
        ]))
        .fetch_in(conn)?;

    let mut volume_by_session = HashMap::new();
    for serie in &series {
        *volume_by_session
            .entry(serie.get_u32(session_step::entity::columns::SESSION.as_ref()))
            .or_insert(0.0) += serie
            .get_u16_nullable(session_step::entity::columns::REPS.as_ref())
            .unwrap_or_default() as f32
            * serie
                .get_f32_nullable(session_step::entity::columns::WEIGHT.as_ref())
                .unwrap_or_default();
    }

    let mut session_list = Vec::new();
    for session in &sessions {
        count += 1;
        time += session.total_elapsed_time;

        let local_volume = volume_by_session.get(&session.date).copied().unwrap_or(0.0);
        volume += local_volume;
        let mut wk_sess = WorkoutSession::from(session);
        wk_sess.volume = local_volume;
        session_list.push(wk_sess);
    }

    let details = WorkoutDetails {
        name: name.to_string(),
        avg_time: if !sessions.is_empty() {
            time / (sessions.len() as u32)
        } else {
            0
        },
        latest_session: latest,
        avg_volume: volume / (sessions.len() as f32),
        session_count: count,
        sessions: session_list,
        enabled: workout.enabled,
        notes: workout.notes,
        steps: workout.steps,
        primary_muscles,
        secondary_muscles,
    };

    Ok(details)
}

/// Enables or disables the workout and queues the change for the devices that have it.
pub fn set_workout_enabled(
    tx: &rusqlite_orm::rusqlite::Transaction,
    workout: &str,
    status: bool,
) -> rusqlite_orm::errors::Result<()> {
    let wk_cl = workout;

    let res = WorkoutRepository::update()
        .where_(Where::Eq(workout::entity::columns::NAME, workout.into()))
        .set(workout::entity::columns::ENABLED, status.into())
        .execute_in(tx)
        .map(|_| ());

    if res.is_ok() {
        // The enabled flag only exists on devices, so Connect is left as it was
        let _ = WorkoutDeviceRepository::update()
            .set(workout_device::entity::columns::PENDING, true.into())
            .where_(Where::And(vec![
                Where::Eq(workout_device::entity::columns::WORKOUT, wk_cl.into()),
                Where::NotEq(
                    workout_device::entity::columns::DEVICE,
                    constants::CONNECT_DEVICE_SERIAL.into(),
                ),
            ]))
            .execute_in(tx);
    }

    res
}

/// Inserts the workout, or replaces the notes and steps of an existing one when editing, and
/// queues it for the devices that already have it.
pub fn store_workout(
    tx: &rusqlite_orm::rusqlite::Transaction,
    workout: &Workout,
    edit: bool,
) -> rusqlite_orm::errors::Result<()> {
    if edit && WorkoutRepository::select_by_id_in(tx, &workout.name)?.is_some() {
        WorkoutRepository::update()
            .set(
                workout::entity::columns::NOTES,
                (if let Some(note) = workout.notes.clone()
                    && !note.is_empty()
                {
                    Some(note)
                } else {
                    None
                })
                .into(),
            )
            .where_(Where::Eq(
                workout::entity::columns::NAME,
                workout.name.clone().into(),
            ))
            .execute_in(tx)?;

        WorkoutStepRepository::delete()
            .where_(Where::Eq(
                workout_step::entity::columns::WORKOUT,
                workout.name.clone().into(),
            ))
            .execute_in(tx)?;

        // Re-send the edited workout to every device that already has it
        WorkoutDeviceRepository::update()
            .set(workout_device::entity::columns::PENDING, true.into())
            .where_(Where::Eq(
                workout_device::entity::columns::WORKOUT,
                workout.name.clone().into(),
            ))
            .execute_in(tx)?;
    } else {
        let mut item = workout.clone();
        WorkoutRepository::insert().item(&mut item).execute_in(tx)?;
    }

    let mut insert = WorkoutStepRepository::insert();
    let mut steps = workout.steps.clone();
    for step in &mut steps {
        step.workout = workout.name.clone();
        // Empty notes are stored as NULL so the exercise notes fallback applies on upload
        if step.notes.as_deref().is_some_and(str::is_empty) {
            step.notes = None;
        }
        insert = insert.item(step);
    }
    insert.execute_in(tx)?;

    Ok(())
}

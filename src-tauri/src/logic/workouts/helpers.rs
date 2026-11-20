use std::collections::HashMap;

use rusqlite_orm::rusqlite::Connection;
use tauri_plugin_log::log::info;

use crate::{
    dao::{
        exercise::Exercise, session::Session, session_step::SessionStep, workout::Workout,
        workout_step::WorkoutStep,
    },
    dto::{
        split_muscles,
        workouts::{WorkoutDetails, WorkoutListItem, WorkoutSession},
    },
};

/// Session stats of a workout, accumulated over its sessions.
#[derive(Default)]
struct WorkoutStats {
    sessions: u32,
    total_time: u32,
    latest_session: Option<u32>,
    total_load: u32,
}

/// Every workout with its session stats (count, average time and load, latest date), by name.
pub(super) async fn workout_list(
    conn: &Connection,
) -> rusqlite_orm::errors::Result<Vec<WorkoutListItem>> {
    let workouts = Workout::get_all(conn).await?;
    let sessions = Session::get_workout_sessions(conn).await?;

    let mut workout_stats: HashMap<u16, WorkoutStats> = workouts
        .iter()
        .map(|w| (w.id, WorkoutStats::default()))
        .collect();

    for s in &sessions {
        let Some(stats) = s.workout.and_then(|id| workout_stats.get_mut(&id)) else {
            continue;
        };
        stats.sessions += 1;
        stats.total_time += s.total_elapsed_time;
        stats.latest_session.get_or_insert(s.date);
        stats.total_load += s.training_load as u32;
    }

    let with_steps = WorkoutStep::get_workouts_with_steps(conn).await?;

    let mut res: Vec<_> = workouts
        .into_iter()
        .map(|workout| {
            let stats = workout_stats.remove(&workout.id).unwrap_or_default();
            WorkoutListItem {
                id: workout.id,
                has_steps: with_steps.contains(&workout.id),
                enabled: workout.enabled,
                sessions: stats.sessions,
                avg_time: stats.total_time.checked_div(stats.sessions).unwrap_or(0),
                avg_load: stats.total_load.checked_div(stats.sessions).unwrap_or(0) as u16,
                latest_session: stats.latest_session,
                name: workout.name,
            }
        })
        .collect();

    res.sort_by(|a, b| a.name.cmp(&b.name));

    Ok(res)
}

/// The workout with its steps, worked muscles and every session with its volume.
pub(super) async fn workout_details(
    conn: &Connection,
    id: u16,
) -> rusqlite_orm::errors::Result<WorkoutDetails> {
    info!("Getting details for workout {}", id);

    let workout = Workout::get_with_steps(conn, id)
        .await?
        .unwrap_or_else(|| panic!("Workout {id} not found"));
    let sessions = Session::get_by_workout(conn, id).await?;

    let mut exercises = Exercise::get_by_workout(conn, id).await?;
    Exercise::fill_muscles(conn, &mut exercises).await?;
    let (primary_muscles, secondary_muscles) = split_muscles(&exercises);

    let latest = sessions.first().map(|latest| latest.date);
    let mut count = 0_u32;
    let mut time = 0_u32;
    let mut volume = 0_f32;

    let volume_by_session = SessionStep::get_volume_by_session(conn, id).await?;

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
        id,
        name: workout.name,
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

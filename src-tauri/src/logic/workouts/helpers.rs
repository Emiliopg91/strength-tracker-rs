use std::collections::{HashMap, HashSet};

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

    let mut workout_stats: HashMap<_, WorkoutStats> = workouts
        .iter()
        .map(|w| (w.name.clone(), WorkoutStats::default()))
        .collect();

    let suggested_workouts: HashSet<_> = workouts
        .iter()
        .filter(|w| w.enabled)
        .map(|w| w.name.clone())
        .collect();

    for s in &sessions {
        let stats = workout_stats.entry(s.name.clone()).or_default();
        stats.sessions += 1;
        stats.total_time += s.total_elapsed_time;
        stats.latest_session.get_or_insert(s.date);
        stats.total_load += s.training_load as u32;
    }

    let with_steps = WorkoutStep::get_workouts_with_steps(conn).await?;

    let mut res: Vec<_> = workout_stats
        .into_iter()
        .map(|(name, stats)| WorkoutListItem {
            has_steps: with_steps.contains(&name),
            enabled: suggested_workouts.contains(&name),
            sessions: stats.sessions,
            avg_time: stats.total_time.checked_div(stats.sessions).unwrap_or(0),
            avg_load: stats.total_load.checked_div(stats.sessions).unwrap_or(0) as u16,
            latest_session: stats.latest_session,
            name,
        })
        .collect();

    res.sort_by(|a, b| a.name.cmp(&b.name));

    Ok(res)
}

/// The workout with its steps, worked muscles and every session with its volume.
pub(super) async fn workout_details(
    conn: &Connection,
    name: &str,
) -> rusqlite_orm::errors::Result<WorkoutDetails> {
    info!("Getting details for workout {}", name);

    let workout = Workout::get_with_steps(conn, name)
        .await?
        .unwrap_or_else(|| panic!("Workout {name} not found"));
    let sessions = Session::get_by_name(conn, name).await?;

    let mut exercises = Exercise::get_by_workout(conn, &workout.name).await?;
    Exercise::fill_muscles(conn, &mut exercises).await?;
    let (primary_muscles, secondary_muscles) = split_muscles(&exercises);

    let latest = sessions.first().map(|latest| latest.date);
    let mut count = 0_u32;
    let mut time = 0_u32;
    let mut volume = 0_f32;

    let volume_by_session = SessionStep::get_volume_by_session(conn, name).await?;

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

use std::collections::HashSet;

use rusqlite_orm::{dao::Repository, database::DatabasePool, types::where_clause::Where};
use rustyfit::profile::typedef::Intensity;

use crate::{
    dao::{
        workout::{Workout, WorkoutRepository},
        workout_device::{WorkoutDevice, WorkoutDeviceRepository, entity::columns as wd_cols},
        workout_step::{WorkoutStep, WorkoutStepRepository, entity::columns as step_cols},
    },
    logic::exercises::helpers::fetch_progression_suggestion,
    tests::common,
    utils::{
        constants,
        strength::{estimate_1rm, percent_load},
    },
};

const BENCH: (u16, u16) = (0, 1);
const WEEK: u32 = 7 * 24 * 3600;
const NOW: u32 = 100 * WEEK;

#[test]
fn estimate_1rm_matches_the_frontend_formula() {
    assert_eq!(estimate_1rm(100.0, 1, false), 100.0);
    assert_eq!(estimate_1rm(90.0, 5, false), 101.0);
    assert_eq!(estimate_1rm(60.0, 12, false), 86.0);
    // Unilateral sets count half their reps
    assert_eq!(estimate_1rm(90.0, 10, true), 101.0);
    // Beyond 20 reps, estimated as 20
    assert_eq!(estimate_1rm(40.0, 30, false), estimate_1rm(40.0, 20, false));
}

#[test]
fn percent_load_rounds_to_the_nearest_kg() {
    assert_eq!(percent_load(120.0, 70), 84.0);
    assert_eq!(percent_load(101.0, 70), 71.0);
    assert_eq!(percent_load(101.0, 50), 51.0);
    assert_eq!(percent_load(103.0, 30), 31.0);
}

async fn log_session(db: &DatabasePool, date: u32, sets: &[(u16, f32)]) {
    let sets = sets.to_vec();
    db.run_in_transaction(async move |tx| {
        let mut session = common::session(date);
        session.steps = sets
            .iter()
            .enumerate()
            .map(|(idx, &(reps, weight))| common::set(date, idx as u8, BENCH, reps, weight))
            .collect();
        common::insert_session(tx, session).await;
        Ok(())
    })
    .await
    .unwrap();
}

/// Programs a workout with progression with a step at `pct`% of the 1RM and a fixed one at
/// `fixed` kg, both of 5 reps, already sent to Connect. Returns its id.
async fn program(db: &DatabasePool, pct: u8, fixed: f32) -> u16 {
    db.run_in_transaction(async move |tx| {
        let mut workout = Workout {
            id: 0,
            name: "Percent".to_string(),
            enabled: true,
            notes: None,
            steps: Vec::new(),
            connect_id: None,
            connect_created_time: None,
            progression: true,
        };
        WorkoutRepository::insert()
            .item(&mut workout)
            .execute_in(tx)
            .await?;

        let step = |idx, weight| {
            WorkoutStep::active(
                workout.id,
                idx,
                Intensity::ACTIVE,
                Some(BENCH),
                weight,
                Some(5),
                None,
                None,
            )
        };
        let mut percent = step(0, 0.0);
        percent.weight = None;
        percent.one_rm_pct = Some(pct);
        let mut steps = vec![percent, step(1, fixed)];
        WorkoutStep::insert_all(tx, &mut steps).await?;
        WorkoutDevice::set_connect_pending(tx, &[workout.id], false).await?;
        Ok(workout.id)
    })
    .await
    .unwrap()
}

async fn refresh(db: &DatabasePool) -> Vec<u16> {
    db.run_in_transaction(async |tx| {
        let keys: HashSet<(u16, u16)> = [BENCH].into();
        Ok(WorkoutStep::refresh_percent_loads(tx, Some(&keys), NOW).await?)
    })
    .await
    .unwrap()
}

async fn step_weight(db: &DatabasePool, workout: u16, idx: u16) -> Option<f32> {
    db.run_in_connection(async move |conn| {
        Ok(WorkoutStepRepository::select()
            .where_(Where::And(vec![
                Where::Eq(step_cols::WORKOUT, workout.into()),
                Where::Eq(step_cols::IDX, idx.into()),
            ]))
            .fetch_one_in(conn)
            .await?
            .and_then(|s| s.weight))
    })
    .await
    .unwrap()
}

async fn connect_pending(db: &DatabasePool, workout: u16) -> bool {
    db.run_in_connection(async move |conn| {
        Ok(WorkoutDeviceRepository::select()
            .where_(Where::And(vec![
                Where::Eq(wd_cols::WORKOUT, workout.into()),
                Where::Eq(wd_cols::DEVICE, constants::CONNECT_DEVICE_SERIAL.into()),
            ]))
            .fetch_one_in(conn)
            .await?
            .is_some_and(|d| d.pending))
    })
    .await
    .unwrap()
}

#[tokio::test]
async fn percent_step_resolves_against_the_best_recent_estimate() {
    let db = common::test_db().await;
    // Out of the window, so its higher estimate is ignored
    log_session(&db, NOW - 9 * WEEK, &[(1, 140.0)]).await;
    log_session(&db, NOW - WEEK, &[(1, 120.0), (5, 90.0)]).await;
    let workout = program(&db, 70, 60.0).await;

    assert_eq!(refresh(&db).await, vec![workout]);
    assert_eq!(step_weight(&db, workout, 0).await, Some(84.0));
    // Fixed loads are left alone
    assert_eq!(step_weight(&db, workout, 1).await, Some(60.0));
    assert!(connect_pending(&db, workout).await);
}

#[tokio::test]
async fn unchanged_percent_load_queues_nothing() {
    let db = common::test_db().await;
    log_session(&db, NOW - WEEK, &[(1, 120.0)]).await;
    let workout = program(&db, 70, 60.0).await;
    refresh(&db).await;
    db.run_in_connection(async move |conn| {
        Ok(WorkoutDevice::set_connect_pending(conn, &[workout], false).await?)
    })
    .await
    .unwrap();

    assert!(refresh(&db).await.is_empty());
    assert!(!connect_pending(&db, workout).await);
}

#[tokio::test]
async fn percent_step_without_recent_sets_goes_unloaded() {
    let db = common::test_db().await;
    log_session(&db, NOW - 9 * WEEK, &[(1, 120.0)]).await;
    let workout = program(&db, 70, 60.0).await;

    assert!(refresh(&db).await.is_empty());
    assert_eq!(step_weight(&db, workout, 0).await, None);
}

#[tokio::test]
async fn percent_steps_stay_out_of_the_progression() {
    let db = common::test_db().await;
    log_session(&db, NOW - WEEK, &[(12, 100.0), (12, 100.0)]).await;
    let workout = program(&db, 70, 100.0).await;
    refresh(&db).await;
    let resolved = step_weight(&db, workout, 0).await;

    let suggestion = db
        .run_in_connection(async |conn| {
            Ok(fetch_progression_suggestion(conn, BENCH.0, BENCH.1).await?)
        })
        .await
        .unwrap()
        .expect("the fixed step progresses");
    db.run_in_transaction(async move |tx| {
        Ok(
            WorkoutStep::apply_progression(
                tx,
                BENCH.0,
                BENCH.1,
                suggestion.weight,
                suggestion.reps,
            )
            .await?,
        )
    })
    .await
    .unwrap();

    assert_eq!(step_weight(&db, workout, 0).await, resolved);
    assert_eq!(step_weight(&db, workout, 1).await, Some(suggestion.weight));
}

#[tokio::test]
async fn unilateral_exercises_estimate_from_half_their_reps() {
    let db = common::test_db().await;
    // 10 reps: 120 kg estimated bilateral, 101 kg unilateral (5 reps a side)
    log_session(&db, NOW - WEEK, &[(10, 90.0)]).await;
    let workout = program(&db, 70, 60.0).await;
    refresh(&db).await;
    assert_eq!(step_weight(&db, workout, 0).await, Some(84.0));

    db.run_in_connection(async |conn| {
        Ok(crate::dao::exercise::Exercise::set_unilateral(conn, BENCH.0, BENCH.1, true).await?)
    })
    .await
    .unwrap();

    assert_eq!(refresh(&db).await, vec![workout]);
    assert_eq!(step_weight(&db, workout, 0).await, Some(71.0));
}

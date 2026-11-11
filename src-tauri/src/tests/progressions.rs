use rusqlite_orm::{dao::Repository, database::DatabasePool, types::where_clause::Where};
use rustyfit::profile::typedef::Intensity;

use crate::{
    dao::{
        exercise::{self, ExerciseRepository},
        session_step::SessionStepRepository,
        workout::{self, Workout, WorkoutRepository},
        workout_step::{StepType, WorkoutStep, WorkoutStepRepository},
    },
    dto::exercises::ProgressionSuggestion,
    logic::exercises::queries::{fetch_progression_suggestion, progression_suggestions},
    tests::common,
};

const BENCH: (u16, u16) = (0, 1);
const CABLE_ROW: (u16, u16) = (23, 20);
const PULL_UP: (u16, u16) = (21, 38);

async fn configure(db: &DatabasePool, exercise: (u16, u16), unilateral: bool, range: &str) {
    let range = range.to_string();
    db.run_in_transaction(move |tx| {
        ExerciseRepository::update()
            .set(exercise::entity::columns::UNILATERAL, unilateral.into())
            .set(exercise::entity::columns::REPS, range.into())
            .where_(Where::And(vec![
                Where::Eq(exercise::entity::columns::CATEGORY, exercise.0.into()),
                Where::Eq(exercise::entity::columns::ID, exercise.1.into()),
            ]))
            .execute_in(tx)?;
        Ok(())
    })
    .await
    .unwrap();
}

async fn disable_progression(db: &DatabasePool, exercise: (u16, u16)) {
    db.run_in_transaction(move |tx| {
        ExerciseRepository::update()
            .set(exercise::entity::columns::PROGRESSION, false.into())
            .where_(Where::And(vec![
                Where::Eq(exercise::entity::columns::CATEGORY, exercise.0.into()),
                Where::Eq(exercise::entity::columns::ID, exercise.1.into()),
            ]))
            .execute_in(tx)?;
        Ok(())
    })
    .await
    .unwrap();
}

/// Logs a session on `date` with one set per `(reps, weight)` pair.
async fn log_session(db: &DatabasePool, date: u32, exercise: (u16, u16), sets: &[(u16, f32)]) {
    let sets = sets.to_vec();
    db.run_in_transaction(move |tx| {
        let mut session = common::session(date);
        session.steps = sets
            .iter()
            .enumerate()
            .map(|(idx, &(reps, weight))| common::set(date, idx as u8, exercise, reps, weight))
            .collect();
        common::insert_session(tx, session);
        Ok(())
    })
    .await
    .unwrap();
}

/// Programs a workout with three sets of `exercise` at `weight` x `reps`.
async fn program(db: &DatabasePool, exercise: (u16, u16), weight: f32, reps: u16) {
    let name = format!("Plan {}-{}", exercise.0, exercise.1);
    db.run_in_transaction(move |tx| {
        WorkoutRepository::insert()
            .item(&mut Workout {
                name: name.clone(),
                enabled: true,
                notes: None,
                steps: Vec::new(),
                connect_id: None,
            })
            .execute_in(tx)?;

        let mut steps: Vec<_> = (0..3)
            .map(|idx| {
                WorkoutStep::active(
                    &name,
                    idx,
                    Intensity::ACTIVE,
                    Some(exercise),
                    weight,
                    Some(reps),
                    None,
                    None,
                )
            })
            .collect();
        let mut insert = WorkoutStepRepository::insert();
        for step in &mut steps {
            insert = insert.item(step);
        }
        insert.execute_in(tx)?;
        Ok(())
    })
    .await
    .unwrap();
}

async fn suggestion(db: &DatabasePool, exercise: (u16, u16)) -> Option<ProgressionSuggestion> {
    db.run_in_connection(move |conn| {
        Ok(fetch_progression_suggestion(conn, exercise.0, exercise.1)?)
    })
    .await
    .unwrap()
}

fn suggested(weight: f32, reps: u16) -> Option<ProgressionSuggestion> {
    Some(ProgressionSuggestion { weight, reps })
}

// Bilateral, weighted

#[tokio::test]
async fn bilateral_top_of_range_adds_weight_and_resets_reps() {
    let db = common::test_db().await;
    configure(&db, BENCH, false, "8-10-12").await;
    log_session(&db, 1, BENCH, &[(12, 100.0), (12, 100.0), (13, 100.0)]).await;
    program(&db, BENCH, 100.0, 12).await;

    assert_eq!(suggestion(&db, BENCH).await, suggested(102.0, 8));
}

#[tokio::test]
async fn bilateral_middle_of_range_targets_max_reps() {
    let db = common::test_db().await;
    configure(&db, BENCH, false, "8-10-12").await;
    log_session(&db, 1, BENCH, &[(10, 100.0), (11, 100.0), (10, 100.0)]).await;
    program(&db, BENCH, 100.0, 10).await;

    assert_eq!(suggestion(&db, BENCH).await, suggested(100.0, 12));
}

#[tokio::test]
async fn bilateral_bottom_of_range_targets_middle_reps() {
    let db = common::test_db().await;
    configure(&db, BENCH, false, "8-10-12").await;
    log_session(&db, 1, BENCH, &[(8, 100.0), (9, 100.0), (8, 100.0)]).await;
    program(&db, BENCH, 100.0, 8).await;

    assert_eq!(suggestion(&db, BENCH).await, suggested(100.0, 10));
}

#[tokio::test]
async fn custom_range_drives_brackets() {
    let db = common::test_db().await;
    configure(&db, BENCH, false, "4-5-6").await;
    log_session(&db, 1, BENCH, &[(6, 120.0), (6, 120.0)]).await;
    program(&db, BENCH, 120.0, 6).await;

    assert_eq!(suggestion(&db, BENCH).await, suggested(122.0, 4));
}

#[tokio::test]
async fn only_latest_session_is_considered() {
    let db = common::test_db().await;
    configure(&db, BENCH, false, "8-10-12").await;
    log_session(&db, 1, BENCH, &[(12, 100.0), (12, 100.0)]).await;
    log_session(&db, 2, BENCH, &[(8, 100.0), (8, 100.0)]).await;
    program(&db, BENCH, 100.0, 8).await;

    assert_eq!(suggestion(&db, BENCH).await, suggested(100.0, 10));
}

#[tokio::test]
async fn sets_in_different_brackets_follow_the_weakest() {
    let db = common::test_db().await;
    configure(&db, BENCH, false, "8-10-12").await;
    log_session(&db, 1, BENCH, &[(12, 100.0), (12, 100.0), (9, 100.0)]).await;
    program(&db, BENCH, 100.0, 8).await;

    assert_eq!(suggestion(&db, BENCH).await, suggested(100.0, 10));
}

#[tokio::test]
async fn missed_programmed_target_does_not_lower_it() {
    let db = common::test_db().await;
    configure(&db, BENCH, false, "8-10-12").await;
    log_session(&db, 1, BENCH, &[(12, 100.0), (12, 100.0), (9, 100.0)]).await;
    program(&db, BENCH, 100.0, 12).await;

    assert_eq!(suggestion(&db, BENCH).await, None);
}

#[tokio::test]
async fn heavier_programmed_weight_is_not_lowered() {
    let db = common::test_db().await;
    configure(&db, BENCH, false, "8-10-12").await;
    log_session(&db, 1, BENCH, &[(12, 100.0), (12, 100.0)]).await;
    program(&db, BENCH, 104.0, 6).await;

    assert_eq!(suggestion(&db, BENCH).await, None);
}

#[tokio::test]
async fn sets_below_range_give_no_suggestion() {
    let db = common::test_db().await;
    configure(&db, BENCH, false, "10-12").await;
    log_session(&db, 1, BENCH, &[(10, 84.0), (10, 84.0), (4, 84.0)]).await;
    program(&db, BENCH, 84.0, 10).await;

    assert_eq!(suggestion(&db, BENCH).await, None);
}

#[tokio::test]
async fn sets_with_different_weights_give_no_suggestion() {
    let db = common::test_db().await;
    configure(&db, BENCH, false, "8-10-12").await;
    log_session(&db, 1, BENCH, &[(12, 60.0), (12, 100.0), (12, 100.0)]).await;
    program(&db, BENCH, 100.0, 12).await;

    assert_eq!(suggestion(&db, BENCH).await, None);
}

// Unilateral, weighted: logged and programmed reps count both sides

#[tokio::test]
async fn unilateral_top_of_range_adds_weight_and_resets_reps() {
    let db = common::test_db().await;
    configure(&db, CABLE_ROW, true, "8-10-12").await;
    log_session(&db, 1, CABLE_ROW, &[(24, 20.0), (24, 20.0)]).await;
    program(&db, CABLE_ROW, 20.0, 24).await;

    assert_eq!(suggestion(&db, CABLE_ROW).await, suggested(22.0, 16));
}

#[tokio::test]
async fn unilateral_middle_of_range_targets_max_reps() {
    let db = common::test_db().await;
    configure(&db, CABLE_ROW, true, "8-10-12").await;
    log_session(&db, 1, CABLE_ROW, &[(20, 20.0), (22, 20.0)]).await;
    program(&db, CABLE_ROW, 20.0, 20).await;

    assert_eq!(suggestion(&db, CABLE_ROW).await, suggested(20.0, 24));
}

#[tokio::test]
async fn unilateral_bottom_of_range_targets_middle_reps() {
    let db = common::test_db().await;
    configure(&db, CABLE_ROW, true, "8-10-12").await;
    log_session(&db, 1, CABLE_ROW, &[(16, 20.0), (18, 20.0)]).await;
    program(&db, CABLE_ROW, 20.0, 16).await;

    assert_eq!(suggestion(&db, CABLE_ROW).await, suggested(20.0, 20));
}

#[tokio::test]
async fn unilateral_reps_are_halved_before_bracketing() {
    let db = common::test_db().await;
    configure(&db, CABLE_ROW, true, "8-10-12").await;
    // 23 total reps is 11 per side: still the middle bracket, not the top one
    log_session(&db, 1, CABLE_ROW, &[(23, 20.0), (23, 20.0)]).await;
    program(&db, CABLE_ROW, 20.0, 20).await;

    assert_eq!(suggestion(&db, CABLE_ROW).await, suggested(20.0, 24));
}

// Bodyweight (no load)

#[tokio::test]
async fn bodyweight_middle_of_range_targets_max_reps_without_weight() {
    let db = common::test_db().await;
    configure(&db, PULL_UP, false, "8-10-12").await;
    log_session(&db, 1, PULL_UP, &[(10, 0.0), (10, 0.0)]).await;
    program(&db, PULL_UP, 0.0, 10).await;

    assert_eq!(suggestion(&db, PULL_UP).await, suggested(0.0, 12));
}

#[tokio::test]
async fn bodyweight_bottom_of_range_targets_middle_reps_without_weight() {
    let db = common::test_db().await;
    configure(&db, PULL_UP, false, "8-10-12").await;
    log_session(&db, 1, PULL_UP, &[(8, 0.0), (9, 0.0)]).await;
    program(&db, PULL_UP, 0.0, 8).await;

    assert_eq!(suggestion(&db, PULL_UP).await, suggested(0.0, 10));
}

#[tokio::test]
async fn bodyweight_top_of_range_adds_weight() {
    let db = common::test_db().await;
    configure(&db, PULL_UP, false, "8-10-12").await;
    log_session(&db, 1, PULL_UP, &[(12, 0.0), (12, 0.0)]).await;
    program(&db, PULL_UP, 0.0, 12).await;

    assert_eq!(suggestion(&db, PULL_UP).await, suggested(2.0, 8));
}

#[tokio::test]
async fn unilateral_bodyweight_top_of_range_adds_weight() {
    let db = common::test_db().await;
    configure(&db, PULL_UP, true, "8-10-12").await;
    log_session(&db, 1, PULL_UP, &[(24, 0.0), (24, 0.0)]).await;
    program(&db, PULL_UP, 0.0, 24).await;

    assert_eq!(suggestion(&db, PULL_UP).await, suggested(2.0, 16));
}

// Missing data / nothing to change

#[tokio::test]
async fn no_logged_sets_give_no_suggestion() {
    let db = common::test_db().await;
    configure(&db, BENCH, false, "8-10-12").await;
    program(&db, BENCH, 100.0, 8).await;

    assert_eq!(suggestion(&db, BENCH).await, None);
}

#[tokio::test]
async fn no_programmed_steps_give_no_suggestion() {
    let db = common::test_db().await;
    configure(&db, BENCH, false, "8-10-12").await;
    log_session(&db, 1, BENCH, &[(12, 100.0), (12, 100.0)]).await;

    assert_eq!(suggestion(&db, BENCH).await, None);
}

#[tokio::test]
async fn steps_already_at_suggestion_give_no_suggestion() {
    let db = common::test_db().await;
    configure(&db, BENCH, false, "8-10-12").await;
    log_session(&db, 1, BENCH, &[(12, 100.0), (12, 100.0)]).await;
    program(&db, BENCH, 102.0, 8).await;

    assert_eq!(suggestion(&db, BENCH).await, None);
}

#[tokio::test]
async fn steps_of_disabled_workouts_give_no_suggestion() {
    let db = common::test_db().await;
    configure(&db, BENCH, false, "8-10-12").await;
    log_session(&db, 1, BENCH, &[(12, 100.0), (11, 100.0)]).await;
    program(&db, BENCH, 100.0, 10).await;
    db.run_in_transaction(move |tx| {
        WorkoutRepository::update()
            .set(workout::entity::columns::ENABLED, false.into())
            .execute_in(tx)?;
        Ok(())
    })
    .await
    .unwrap();

    assert_eq!(suggestion(&db, BENCH).await, None);
}

#[tokio::test]
async fn disabled_progression_gives_no_suggestion() {
    let db = common::test_db().await;
    configure(&db, BENCH, false, "8-10-12").await;
    disable_progression(&db, BENCH).await;
    log_session(&db, 1, BENCH, &[(12, 100.0), (12, 100.0)]).await;
    program(&db, BENCH, 100.0, 12).await;

    assert_eq!(suggestion(&db, BENCH).await, None);
}

// Warm-ups and the like don't count

#[tokio::test]
async fn light_warmup_set_in_session_does_not_block_the_suggestion() {
    let db = common::test_db().await;
    configure(&db, BENCH, false, "8-10-12").await;
    log_session(&db, 1, BENCH, &[(12, 100.0), (12, 100.0)]).await;
    db.run_in_transaction(move |tx| {
        let mut warmup = common::set(1, 2, BENCH, 15, 40.0);
        warmup.kind = StepType::Warmup;
        SessionStepRepository::insert()
            .item(&mut warmup)
            .execute_in(tx)?;
        Ok(())
    })
    .await
    .unwrap();
    program(&db, BENCH, 100.0, 12).await;

    assert_eq!(suggestion(&db, BENCH).await, suggested(102.0, 8));
}

#[tokio::test]
async fn programmed_warmup_alone_does_not_trigger_the_suggestion() {
    let db = common::test_db().await;
    configure(&db, BENCH, false, "8-10-12").await;
    log_session(&db, 1, BENCH, &[(10, 100.0), (10, 100.0)]).await;
    program(&db, BENCH, 100.0, 12).await;
    db.run_in_transaction(move |tx| {
        WorkoutStepRepository::insert()
            .item(&mut WorkoutStep::active(
                "Plan 0-1",
                3,
                Intensity::WARMUP,
                Some(BENCH),
                40.0,
                Some(15),
                None,
                None,
            ))
            .execute_in(tx)?;
        Ok(())
    })
    .await
    .unwrap();

    assert_eq!(suggestion(&db, BENCH).await, None);
}

#[tokio::test]
async fn progression_suggestions_lists_only_exercises_with_a_suggestion() {
    let db = common::test_db().await;
    configure(&db, BENCH, false, "8-10-12").await;
    log_session(&db, 1, BENCH, &[(12, 100.0), (12, 100.0), (12, 100.0)]).await;
    program(&db, BENCH, 100.0, 12).await;
    // Logged but never programmed: nothing to raise
    configure(&db, CABLE_ROW, false, "8-10-12").await;
    log_session(&db, 2, CABLE_ROW, &[(12, 50.0), (12, 50.0)]).await;

    let suggested = db
        .run_in_connection(move |conn| Ok(progression_suggestions(conn)?))
        .await
        .unwrap();

    assert_eq!(suggested, std::collections::HashSet::from([BENCH]));
    assert!(suggestion(&db, CABLE_ROW).await.is_none());
}

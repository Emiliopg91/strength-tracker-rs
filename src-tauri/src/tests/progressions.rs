use rusqlite_orm::{dao::Repository, database::DatabasePool, types::where_clause::Where};

use crate::{
    dao::{
        exercise::{self, ExerciseRepository},
        workout::{Workout, WorkoutRepository},
        workout_step::{WorkoutStep, WorkoutStepRepository},
    },
    dto::exercises::ProgressionSuggestion,
    logic::exercises::fetch_progression_suggestion,
    tests::common,
};

const BENCH: (u16, u16) = (0, 1);
const CABLE_ROW: (u16, u16) = (23, 20);
const PULL_UP: (u16, u16) = (21, 38);

fn configure(db: &DatabasePool, exercise: (u16, u16), unilateral: bool, range: &str) {
    db.run_in_transaction(|tx| {
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
    .unwrap();
}

fn disable_progression(db: &DatabasePool, exercise: (u16, u16)) {
    db.run_in_transaction(|tx| {
        ExerciseRepository::update()
            .set(exercise::entity::columns::PROGRESSION, false.into())
            .where_(Where::And(vec![
                Where::Eq(exercise::entity::columns::CATEGORY, exercise.0.into()),
                Where::Eq(exercise::entity::columns::ID, exercise.1.into()),
            ]))
            .execute_in(tx)?;
        Ok(())
    })
    .unwrap();
}

/// Logs a session on `date` with one set per `(reps, weight)` pair.
fn log_session(db: &DatabasePool, date: u32, exercise: (u16, u16), sets: &[(u16, f32)]) {
    db.run_in_transaction(|tx| {
        let mut session = common::session(date);
        session.sets = sets
            .iter()
            .enumerate()
            .map(|(idx, &(reps, weight))| common::set(date, idx as u8, exercise, reps, weight))
            .collect();
        common::insert_session(tx, session);
        Ok(())
    })
    .unwrap();
}

/// Programs a workout with three sets of `exercise` at `weight` x `reps`.
fn program(db: &DatabasePool, exercise: (u16, u16), weight: f32, reps: u16) {
    let name = format!("Plan {}-{}", exercise.0, exercise.1);
    db.run_in_transaction(|tx| {
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
                WorkoutStep::exercise(
                    &name,
                    idx,
                    exercise.0,
                    exercise.1,
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
    .unwrap();
}

fn suggestion(db: &DatabasePool, exercise: (u16, u16)) -> Option<ProgressionSuggestion> {
    db.run_in_connection(|conn| Ok(fetch_progression_suggestion(conn, exercise.0, exercise.1)?))
        .unwrap()
}

fn suggested(weight: f32, reps: u16) -> Option<ProgressionSuggestion> {
    Some(ProgressionSuggestion { weight, reps })
}

// Bilateral, weighted

#[test]
fn bilateral_top_of_range_adds_weight_and_resets_reps() {
    let db = common::test_db();
    configure(&db, BENCH, false, "8-10-12");
    log_session(&db, 1, BENCH, &[(12, 100.0), (12, 100.0), (13, 100.0)]);
    program(&db, BENCH, 100.0, 12);

    assert_eq!(suggestion(&db, BENCH), suggested(102.0, 8));
}

#[test]
fn bilateral_middle_of_range_targets_max_reps() {
    let db = common::test_db();
    configure(&db, BENCH, false, "8-10-12");
    log_session(&db, 1, BENCH, &[(10, 100.0), (11, 100.0), (10, 100.0)]);
    program(&db, BENCH, 100.0, 10);

    assert_eq!(suggestion(&db, BENCH), suggested(100.0, 12));
}

#[test]
fn bilateral_bottom_of_range_targets_middle_reps() {
    let db = common::test_db();
    configure(&db, BENCH, false, "8-10-12");
    log_session(&db, 1, BENCH, &[(8, 100.0), (9, 100.0), (8, 100.0)]);
    program(&db, BENCH, 100.0, 8);

    assert_eq!(suggestion(&db, BENCH), suggested(100.0, 10));
}

#[test]
fn custom_range_drives_brackets() {
    let db = common::test_db();
    configure(&db, BENCH, false, "4-5-6");
    log_session(&db, 1, BENCH, &[(6, 120.0), (6, 120.0)]);
    program(&db, BENCH, 120.0, 6);

    assert_eq!(suggestion(&db, BENCH), suggested(122.0, 4));
}

#[test]
fn only_latest_session_is_considered() {
    let db = common::test_db();
    configure(&db, BENCH, false, "8-10-12");
    log_session(&db, 1, BENCH, &[(12, 100.0), (12, 100.0)]);
    log_session(&db, 2, BENCH, &[(8, 100.0), (8, 100.0)]);
    program(&db, BENCH, 100.0, 8);

    assert_eq!(suggestion(&db, BENCH), suggested(100.0, 10));
}

#[test]
fn sets_in_different_brackets_follow_the_weakest() {
    let db = common::test_db();
    configure(&db, BENCH, false, "8-10-12");
    log_session(&db, 1, BENCH, &[(12, 100.0), (12, 100.0), (9, 100.0)]);
    program(&db, BENCH, 100.0, 8);

    assert_eq!(suggestion(&db, BENCH), suggested(100.0, 10));
}

#[test]
fn missed_programmed_target_does_not_lower_it() {
    let db = common::test_db();
    configure(&db, BENCH, false, "8-10-12");
    log_session(&db, 1, BENCH, &[(12, 100.0), (12, 100.0), (9, 100.0)]);
    program(&db, BENCH, 100.0, 12);

    assert_eq!(suggestion(&db, BENCH), None);
}

#[test]
fn heavier_programmed_weight_is_not_lowered() {
    let db = common::test_db();
    configure(&db, BENCH, false, "8-10-12");
    log_session(&db, 1, BENCH, &[(12, 100.0), (12, 100.0)]);
    program(&db, BENCH, 104.0, 6);

    assert_eq!(suggestion(&db, BENCH), None);
}

#[test]
fn sets_below_range_give_no_suggestion() {
    let db = common::test_db();
    configure(&db, BENCH, false, "10-12");
    log_session(&db, 1, BENCH, &[(10, 84.0), (10, 84.0), (4, 84.0)]);
    program(&db, BENCH, 84.0, 10);

    assert_eq!(suggestion(&db, BENCH), None);
}

#[test]
fn sets_with_different_weights_give_no_suggestion() {
    let db = common::test_db();
    configure(&db, BENCH, false, "8-10-12");
    log_session(&db, 1, BENCH, &[(12, 60.0), (12, 100.0), (12, 100.0)]);
    program(&db, BENCH, 100.0, 12);

    assert_eq!(suggestion(&db, BENCH), None);
}

// Unilateral, weighted: logged and programmed reps count both sides

#[test]
fn unilateral_top_of_range_adds_weight_and_resets_reps() {
    let db = common::test_db();
    configure(&db, CABLE_ROW, true, "8-10-12");
    log_session(&db, 1, CABLE_ROW, &[(24, 20.0), (24, 20.0)]);
    program(&db, CABLE_ROW, 20.0, 24);

    assert_eq!(suggestion(&db, CABLE_ROW), suggested(22.0, 16));
}

#[test]
fn unilateral_middle_of_range_targets_max_reps() {
    let db = common::test_db();
    configure(&db, CABLE_ROW, true, "8-10-12");
    log_session(&db, 1, CABLE_ROW, &[(20, 20.0), (22, 20.0)]);
    program(&db, CABLE_ROW, 20.0, 20);

    assert_eq!(suggestion(&db, CABLE_ROW), suggested(20.0, 24));
}

#[test]
fn unilateral_bottom_of_range_targets_middle_reps() {
    let db = common::test_db();
    configure(&db, CABLE_ROW, true, "8-10-12");
    log_session(&db, 1, CABLE_ROW, &[(16, 20.0), (18, 20.0)]);
    program(&db, CABLE_ROW, 20.0, 16);

    assert_eq!(suggestion(&db, CABLE_ROW), suggested(20.0, 20));
}

#[test]
fn unilateral_reps_are_halved_before_bracketing() {
    let db = common::test_db();
    configure(&db, CABLE_ROW, true, "8-10-12");
    // 23 total reps is 11 per side: still the middle bracket, not the top one
    log_session(&db, 1, CABLE_ROW, &[(23, 20.0), (23, 20.0)]);
    program(&db, CABLE_ROW, 20.0, 20);

    assert_eq!(suggestion(&db, CABLE_ROW), suggested(20.0, 24));
}

// Bodyweight (no load)

#[test]
fn bodyweight_middle_of_range_targets_max_reps_without_weight() {
    let db = common::test_db();
    configure(&db, PULL_UP, false, "8-10-12");
    log_session(&db, 1, PULL_UP, &[(10, 0.0), (10, 0.0)]);
    program(&db, PULL_UP, 0.0, 10);

    assert_eq!(suggestion(&db, PULL_UP), suggested(0.0, 12));
}

#[test]
fn bodyweight_bottom_of_range_targets_middle_reps_without_weight() {
    let db = common::test_db();
    configure(&db, PULL_UP, false, "8-10-12");
    log_session(&db, 1, PULL_UP, &[(8, 0.0), (9, 0.0)]);
    program(&db, PULL_UP, 0.0, 8);

    assert_eq!(suggestion(&db, PULL_UP), suggested(0.0, 10));
}

#[test]
fn bodyweight_top_of_range_adds_weight() {
    let db = common::test_db();
    configure(&db, PULL_UP, false, "8-10-12");
    log_session(&db, 1, PULL_UP, &[(12, 0.0), (12, 0.0)]);
    program(&db, PULL_UP, 0.0, 12);

    assert_eq!(suggestion(&db, PULL_UP), suggested(2.0, 8));
}

#[test]
fn unilateral_bodyweight_top_of_range_adds_weight() {
    let db = common::test_db();
    configure(&db, PULL_UP, true, "8-10-12");
    log_session(&db, 1, PULL_UP, &[(24, 0.0), (24, 0.0)]);
    program(&db, PULL_UP, 0.0, 24);

    assert_eq!(suggestion(&db, PULL_UP), suggested(2.0, 16));
}

// Missing data / nothing to change

#[test]
fn no_logged_sets_give_no_suggestion() {
    let db = common::test_db();
    configure(&db, BENCH, false, "8-10-12");
    program(&db, BENCH, 100.0, 8);

    assert_eq!(suggestion(&db, BENCH), None);
}

#[test]
fn no_programmed_steps_give_no_suggestion() {
    let db = common::test_db();
    configure(&db, BENCH, false, "8-10-12");
    log_session(&db, 1, BENCH, &[(12, 100.0), (12, 100.0)]);

    assert_eq!(suggestion(&db, BENCH), None);
}

#[test]
fn steps_already_at_suggestion_give_no_suggestion() {
    let db = common::test_db();
    configure(&db, BENCH, false, "8-10-12");
    log_session(&db, 1, BENCH, &[(12, 100.0), (12, 100.0)]);
    program(&db, BENCH, 102.0, 8);

    assert_eq!(suggestion(&db, BENCH), None);
}

#[test]
fn disabled_progression_gives_no_suggestion() {
    let db = common::test_db();
    configure(&db, BENCH, false, "8-10-12");
    disable_progression(&db, BENCH);
    log_session(&db, 1, BENCH, &[(12, 100.0), (12, 100.0)]);
    program(&db, BENCH, 100.0, 12);

    assert_eq!(suggestion(&db, BENCH), None);
}

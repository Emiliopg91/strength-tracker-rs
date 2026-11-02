use rusqlite_orm::{dao::Repository, database::DatabasePool};

use crate::{
    dao::{
        device::{Device, DeviceRepository},
        exercise::ExerciseRepository,
        workout::{Workout, WorkoutRepository},
        workout_device::{WorkoutDevice, WorkoutDeviceRepository},
        workout_step::{WorkoutStep, WorkoutStepRepository},
    },
    logic::exercises::update_exercise_notes,
    tests::common,
};

const BENCH: (u16, u16) = (0, 1);
const CABLE_ROW: (u16, u16) = (23, 20);
const WATCH: &str = "watch";
const EDGE: &str = "edge";

/// Programs an enabled workout with one step per `(exercise, notes)` pair, linked to both devices.
fn program(db: &DatabasePool, name: &str, steps: &[((u16, u16), Option<&str>)]) {
    db.run_in_transaction(|tx| {
        WorkoutRepository::insert()
            .item(&mut Workout {
                name: name.to_string(),
                enabled: true,
                notes: None,
                steps: Vec::new(),
                connect_id: None,
            })
            .execute_in(tx)?;

        let mut steps: Vec<_> = steps
            .iter()
            .enumerate()
            .map(|(idx, (exercise, notes))| {
                WorkoutStep::exercise(
                    name,
                    idx as u16,
                    exercise.0,
                    exercise.1,
                    60.0,
                    Some(10),
                    None,
                    notes.map(str::to_string),
                )
            })
            .collect();
        let mut insert = WorkoutStepRepository::insert();
        for step in &mut steps {
            insert = insert.item(step);
        }
        insert.execute_in(tx)?;

        for device in [WATCH, EDGE] {
            WorkoutDeviceRepository::insert()
                .item(&mut WorkoutDevice {
                    workout: name.to_string(),
                    device: device.to_string(),
                    pending: false,
                })
                .execute_in(tx)?;
        }
        Ok(())
    })
    .unwrap();
}

fn setup() -> DatabasePool {
    let db = common::test_db();
    db.run_in_transaction(|tx| {
        for serial in [WATCH, EDGE] {
            DeviceRepository::insert()
                .item(&mut Device {
                    serial: serial.to_string(),
                    model: "MODEL".to_string(),
                    last_activity_sync: None,
                    last_body_metric_sync: None,
                    usb: true,
                    connect: false,
                    strength_workout: true,
                })
                .execute_in(tx)?;
        }
        Ok(())
    })
    .unwrap();
    db
}

fn disable(db: &DatabasePool, name: &str) {
    db.run_in_transaction(|tx| {
        let mut workout = WorkoutRepository::select_by_id_in(tx, name)?.unwrap();
        workout.enabled = false;
        Ok(workout.update_by_id_in(tx)?)
    })
    .unwrap();
}

fn save_notes(db: &DatabasePool, exercise: (u16, u16), notes: Option<&str>) -> Vec<String> {
    let mut workouts = db
        .run_in_transaction(|tx| {
            Ok(update_exercise_notes(
                tx,
                exercise.0,
                exercise.1,
                notes.map(str::to_string),
            )?)
        })
        .unwrap();
    workouts.sort();
    workouts
}

fn pending(db: &DatabasePool, workout: &str, device: &str) -> bool {
    WorkoutDeviceRepository::select_by_id(db, workout, device)
        .unwrap()
        .unwrap()
        .pending
}

fn all_pending(db: &DatabasePool, workout: &str) -> bool {
    pending(db, workout, WATCH) && pending(db, workout, EDGE)
}

fn none_pending(db: &DatabasePool, workout: &str) -> bool {
    !pending(db, workout, WATCH) && !pending(db, workout, EDGE)
}

fn exercise_notes(db: &DatabasePool, exercise: (u16, u16)) -> Option<String> {
    ExerciseRepository::select_by_id(db, exercise.0, exercise.1)
        .unwrap()
        .unwrap()
        .notes
}

#[test]
fn step_without_notes_queues_every_device() {
    let db = setup();
    program(&db, "Push", &[(BENCH, None)]);

    let workouts = save_notes(&db, BENCH, Some("Elbows in"));

    assert_eq!(workouts, ["Push"]);
    assert!(all_pending(&db, "Push"));
    assert_eq!(exercise_notes(&db, BENCH).as_deref(), Some("Elbows in"));
}

#[test]
fn step_with_empty_notes_counts_as_without_notes() {
    let db = setup();
    program(&db, "Push", &[(BENCH, Some(""))]);

    assert_eq!(save_notes(&db, BENCH, Some("Elbows in")), ["Push"]);
    assert!(all_pending(&db, "Push"));
}

#[test]
fn steps_with_own_notes_queue_nothing_but_notes_are_stored() {
    let db = setup();
    program(&db, "Push", &[(BENCH, Some("Slow eccentric"))]);

    assert!(save_notes(&db, BENCH, Some("Elbows in")).is_empty());
    assert!(none_pending(&db, "Push"));
    assert_eq!(exercise_notes(&db, BENCH).as_deref(), Some("Elbows in"));
}

#[test]
fn one_step_without_notes_is_enough() {
    let db = setup();
    program(
        &db,
        "Push",
        &[(BENCH, Some("Slow eccentric")), (BENCH, None)],
    );

    assert_eq!(save_notes(&db, BENCH, Some("Elbows in")), ["Push"]);
    assert!(all_pending(&db, "Push"));
}

#[test]
fn other_exercises_and_workouts_are_not_touched() {
    let db = setup();
    program(&db, "Push", &[(BENCH, None), (CABLE_ROW, None)]);
    program(&db, "Pull", &[(CABLE_ROW, None)]);

    assert_eq!(save_notes(&db, BENCH, Some("Elbows in")), ["Push"]);
    assert!(all_pending(&db, "Push"));
    assert!(none_pending(&db, "Pull"));
    assert_eq!(exercise_notes(&db, CABLE_ROW), None);
}

#[test]
fn every_workout_inheriting_the_notes_is_queued() {
    let db = setup();
    program(&db, "Push", &[(BENCH, None), (BENCH, None)]);
    program(&db, "Full body", &[(BENCH, None)]);

    assert_eq!(
        save_notes(&db, BENCH, Some("Elbows in")),
        ["Full body", "Push"]
    );
    assert!(all_pending(&db, "Push"));
    assert!(all_pending(&db, "Full body"));
}

#[test]
fn clearing_notes_queues_workouts() {
    let db = setup();
    program(&db, "Push", &[(BENCH, None)]);
    save_notes(&db, BENCH, Some("Elbows in"));
    db.run_in_transaction(|tx| {
        let mut device = WorkoutDeviceRepository::select_by_id_in(tx, "Push", WATCH)?.unwrap();
        device.pending = false;
        Ok(device.update_by_id_in(tx)?)
    })
    .unwrap();

    assert_eq!(save_notes(&db, BENCH, None), ["Push"]);
    assert!(all_pending(&db, "Push"));
    assert_eq!(exercise_notes(&db, BENCH), None);
}

#[test]
fn disabled_workout_is_returned_but_not_queued() {
    let db = setup();
    program(&db, "Push", &[(BENCH, None)]);
    disable(&db, "Push");

    assert_eq!(save_notes(&db, BENCH, Some("Elbows in")), ["Push"]);
    assert!(none_pending(&db, "Push"));
}

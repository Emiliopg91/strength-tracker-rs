use rusqlite_orm::{dao::Repository, database::DatabasePool};
use rustyfit::profile::typedef::Intensity;

use crate::{
    dao::{
        device::{Device, DeviceRepository},
        exercise::ExerciseRepository,
        workout::{Workout, WorkoutRepository},
        workout_device::{WorkoutDevice, WorkoutDeviceRepository},
        workout_step::{WorkoutStep, WorkoutStepRepository},
    },
    logic::exercises::queries::update_exercise_notes,
    tests::common,
};

const BENCH: (u16, u16) = (0, 1);
const CABLE_ROW: (u16, u16) = (23, 20);
const WATCH: &str = "watch";
const EDGE: &str = "edge";

/// Programs an enabled workout with one step per `(exercise, notes)` pair, linked to both devices.
async fn program(db: &DatabasePool, name: &str, steps: &[((u16, u16), Option<&str>)]) {
    let name = name.to_string();
    let steps: Vec<((u16, u16), Option<String>)> = steps
        .iter()
        .map(|(exercise, notes)| (*exercise, notes.map(str::to_string)))
        .collect();
    db.run_in_transaction(move |tx| {
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
                WorkoutStep::active(
                    &name,
                    idx as u16,
                    Intensity::ACTIVE,
                    Some(*exercise),
                    60.0,
                    Some(10),
                    None,
                    notes.clone(),
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
    .await
    .unwrap();
}

async fn setup() -> DatabasePool {
    let db = common::test_db().await;
    db.run_in_transaction(move |tx| {
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
    .await
    .unwrap();
    db
}

async fn disable(db: &DatabasePool, name: &str) {
    let name = name.to_string();
    db.run_in_transaction(move |tx| {
        let mut workout = WorkoutRepository::select_by_id_in(tx, &name)?.unwrap();
        workout.enabled = false;
        Ok(workout.update_by_id_in(tx)?)
    })
    .await
    .unwrap();
}

async fn save_notes(db: &DatabasePool, exercise: (u16, u16), notes: Option<&str>) -> Vec<String> {
    let notes = notes.map(str::to_string);
    let mut workouts = db
        .run_in_transaction(move |tx| Ok(update_exercise_notes(tx, exercise.0, exercise.1, notes)?))
        .await
        .unwrap();
    workouts.sort();
    workouts
}

async fn pending(db: &DatabasePool, workout: &str, device: &str) -> bool {
    WorkoutDeviceRepository::select_by_id(db, workout, device)
        .await
        .unwrap()
        .unwrap()
        .pending
}

async fn all_pending(db: &DatabasePool, workout: &str) -> bool {
    pending(db, workout, WATCH).await && pending(db, workout, EDGE).await
}

async fn none_pending(db: &DatabasePool, workout: &str) -> bool {
    !pending(db, workout, WATCH).await && !pending(db, workout, EDGE).await
}

async fn exercise_notes(db: &DatabasePool, exercise: (u16, u16)) -> Option<String> {
    ExerciseRepository::select_by_id(db, exercise.0, exercise.1)
        .await
        .unwrap()
        .unwrap()
        .notes
}

#[tokio::test]
async fn step_without_notes_queues_every_device() {
    let db = setup().await;
    program(&db, "Push", &[(BENCH, None)]).await;

    let workouts = save_notes(&db, BENCH, Some("Elbows in")).await;

    assert_eq!(workouts, ["Push"]);
    assert!(all_pending(&db, "Push").await);
    assert_eq!(
        exercise_notes(&db, BENCH).await.as_deref(),
        Some("Elbows in")
    );
}

#[tokio::test]
async fn step_with_empty_notes_counts_as_without_notes() {
    let db = setup().await;
    program(&db, "Push", &[(BENCH, Some(""))]).await;

    assert_eq!(save_notes(&db, BENCH, Some("Elbows in")).await, ["Push"]);
    assert!(all_pending(&db, "Push").await);
}

#[tokio::test]
async fn steps_with_own_notes_queue_nothing_but_notes_are_stored() {
    let db = setup().await;
    program(&db, "Push", &[(BENCH, Some("Slow eccentric"))]).await;

    assert!(save_notes(&db, BENCH, Some("Elbows in")).await.is_empty());
    assert!(none_pending(&db, "Push").await);
    assert_eq!(
        exercise_notes(&db, BENCH).await.as_deref(),
        Some("Elbows in")
    );
}

#[tokio::test]
async fn one_step_without_notes_is_enough() {
    let db = setup().await;
    program(
        &db,
        "Push",
        &[(BENCH, Some("Slow eccentric")), (BENCH, None)],
    )
    .await;

    assert_eq!(save_notes(&db, BENCH, Some("Elbows in")).await, ["Push"]);
    assert!(all_pending(&db, "Push").await);
}

#[tokio::test]
async fn other_exercises_and_workouts_are_not_touched() {
    let db = setup().await;
    program(&db, "Push", &[(BENCH, None), (CABLE_ROW, None)]).await;
    program(&db, "Pull", &[(CABLE_ROW, None)]).await;

    assert_eq!(save_notes(&db, BENCH, Some("Elbows in")).await, ["Push"]);
    assert!(all_pending(&db, "Push").await);
    assert!(none_pending(&db, "Pull").await);
    assert_eq!(exercise_notes(&db, CABLE_ROW).await, None);
}

#[tokio::test]
async fn every_workout_inheriting_the_notes_is_queued() {
    let db = setup().await;
    program(&db, "Push", &[(BENCH, None), (BENCH, None)]).await;
    program(&db, "Full body", &[(BENCH, None)]).await;

    assert_eq!(
        save_notes(&db, BENCH, Some("Elbows in")).await,
        ["Full body", "Push"]
    );
    assert!(all_pending(&db, "Push").await);
    assert!(all_pending(&db, "Full body").await);
}

#[tokio::test]
async fn clearing_notes_queues_workouts() {
    let db = setup().await;
    program(&db, "Push", &[(BENCH, None)]).await;
    save_notes(&db, BENCH, Some("Elbows in")).await;
    db.run_in_transaction(move |tx| {
        let mut device = WorkoutDeviceRepository::select_by_id_in(tx, "Push", WATCH)?.unwrap();
        device.pending = false;
        Ok(device.update_by_id_in(tx)?)
    })
    .await
    .unwrap();

    assert_eq!(save_notes(&db, BENCH, None).await, ["Push"]);
    assert!(all_pending(&db, "Push").await);
    assert_eq!(exercise_notes(&db, BENCH).await, None);
}

#[tokio::test]
async fn disabled_workout_is_returned_but_not_queued() {
    let db = setup().await;
    program(&db, "Push", &[(BENCH, None)]).await;
    disable(&db, "Push").await;

    assert_eq!(save_notes(&db, BENCH, Some("Elbows in")).await, ["Push"]);
    assert!(none_pending(&db, "Push").await);
}

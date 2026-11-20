use rusqlite_orm::{dao::Repository, database::DatabasePool};
use rustyfit::profile::typedef::Intensity;

use crate::{
    dao::{
        device::{Device, DeviceRepository},
        exercise::{Exercise, ExerciseRepository},
        workout::{Workout, WorkoutRepository},
        workout_device::{WorkoutDevice, WorkoutDeviceRepository},
        workout_step::{WorkoutStep, WorkoutStepRepository},
    },
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
    db.run_in_transaction(async move |tx| {
        let mut workout = Workout {
            id: 0,
            name: name.clone(),
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
        let id = workout.id;

        let mut steps: Vec<_> = steps
            .iter()
            .enumerate()
            .map(|(idx, (exercise, notes))| {
                WorkoutStep::active(
                    id,
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
        insert.execute_in(tx).await?;

        for device in [WATCH, EDGE] {
            WorkoutDeviceRepository::insert()
                .item(&mut WorkoutDevice {
                    workout: id,
                    device: device.to_string(),
                    pending: false,
                })
                .execute_in(tx)
                .await?;
        }
        Ok(())
    })
    .await
    .unwrap();
}

async fn setup() -> DatabasePool {
    let db = common::test_db().await;
    db.run_in_transaction(async move |tx| {
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
                .execute_in(tx)
                .await?;
        }
        Ok(())
    })
    .await
    .unwrap();
    db
}

async fn disable(db: &DatabasePool, name: &str) {
    let name = name.to_string();
    db.run_in_transaction(async move |tx| {
        let mut workout = WorkoutRepository::select_by_name_in(tx, &name)
            .await?
            .unwrap();
        workout.enabled = false;
        Ok(workout.update_by_id_in(tx).await?)
    })
    .await
    .unwrap();
}

async fn disable_progression(db: &DatabasePool, name: &str) {
    let name = name.to_string();
    db.run_in_transaction(async move |tx| {
        let mut workout = WorkoutRepository::select_by_name_in(tx, &name)
            .await?
            .unwrap();
        workout.progression = false;
        Ok(workout.update_by_id_in(tx).await?)
    })
    .await
    .unwrap();
}

async fn save_notes(db: &DatabasePool, exercise: (u16, u16), notes: Option<&str>) -> Vec<String> {
    let notes = notes.map(str::to_string);
    let mut workouts = db
        .run_in_transaction(async move |tx| {
            let mut names = Vec::new();
            for id in Exercise::update_notes(tx, exercise.0, exercise.1, notes).await? {
                names.push(Workout::get(tx, id).await?.unwrap().name);
            }
            Ok(names)
        })
        .await
        .unwrap();
    workouts.sort();
    workouts
}

async fn pending(db: &DatabasePool, workout: &str, device: &str) -> bool {
    let workout = common::workout_id_in(db, workout).await;
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

    assert_eq!(
        save_notes(&db, BENCH, Some("Elbows in")).await,
        Vec::<String>::new()
    );
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
    db.run_in_transaction(async move |tx| {
        let push = common::workout_id(tx, "Push").await;
        let mut device = WorkoutDeviceRepository::select_by_id_in(tx, push, WATCH)
            .await?
            .unwrap();
        device.pending = false;
        Ok(device.update_by_id_in(tx).await?)
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

#[tokio::test]
async fn workout_without_progression_is_neither_returned_nor_queued() {
    let db = setup().await;
    program(&db, "Push", &[(BENCH, None)]).await;
    program(&db, "Fixed", &[(BENCH, None)]).await;
    disable_progression(&db, "Fixed").await;

    assert_eq!(save_notes(&db, BENCH, Some("Elbows in")).await, ["Push"]);
    assert!(all_pending(&db, "Push").await);
    assert!(none_pending(&db, "Fixed").await);
}

#[tokio::test]
async fn device_workout_inherits_notes_only_with_progression() {
    let db = setup().await;
    program(&db, "Push", &[(BENCH, None)]).await;
    program(&db, "Fixed", &[(BENCH, None)]).await;
    disable_progression(&db, "Fixed").await;
    save_notes(&db, BENCH, Some("Elbows in")).await;

    let step_notes = |name: &'static str| {
        let db = &db;
        async move {
            db.run_in_connection(async move |conn| {
                let id = Workout::get_id(conn, name).await?.unwrap();
                Ok(Workout::get_for_device(conn, id).await?.unwrap().steps[0]
                    .notes
                    .clone())
            })
            .await
            .unwrap()
        }
    };
    assert_eq!(step_notes("Push").await.as_deref(), Some("Elbows in"));
    assert_eq!(step_notes("Fixed").await, None);
}

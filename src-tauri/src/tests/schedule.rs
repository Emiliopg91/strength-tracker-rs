use rusqlite_orm::{dao::Repository, database::DatabasePool};

use crate::{
    dao::{
        device::{Device, DeviceRepository},
        workout::{Workout, WorkoutRepository},
        workout_device::{WorkoutDevice, WorkoutDeviceRepository},
        workout_planning::{WorkoutPlanning, WorkoutPlanningRepository},
    },
    tests::common,
    utils::constants,
};

const DAY: u32 = 86_400;
const FROM: u32 = 1_794_783_600;

/// Ids of the workouts "a" and "b", inserted in that order on an empty database
const A: u16 = 1;
const B: u16 = 2;

fn id(name: &str) -> u16 {
    match name {
        "a" => A,
        "b" => B,
        _ => unreachable!("unknown workout {name}"),
    }
}

async fn setup(plans: &[(u32, &'static str)]) -> DatabasePool {
    let db = common::test_db().await;
    let plans = plans.to_vec();
    db.run_in_transaction(async move |tx| {
        for (name, id, created) in [
            ("a", "1746212841", Some(1_793_401_668)),
            ("b", "1746213483", None),
        ] {
            WorkoutRepository::insert()
                .item(&mut Workout {
                    id: 0,
                    name: name.to_string(),
                    enabled: true,
                    notes: None,
                    steps: Vec::new(),
                    connect_id: Some(id.to_string()),
                    connect_created_time: created,
                    progression: true,
                })
                .execute_in(tx)
                .await?;
        }
        for (date, workout) in plans {
            WorkoutPlanningRepository::insert()
                .item(&mut WorkoutPlanning {
                    date,
                    workout: id(workout),
                    session: None,
                    connect_id: None,
                    workout_obj: None,
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

async fn between(db: &DatabasePool, from: u32, to: u32) -> Vec<WorkoutPlanning> {
    db.run_in_connection(async move |conn| Ok(WorkoutPlanning::get_between(conn, from, to).await?))
        .await
        .unwrap()
}

#[tokio::test]
async fn only_plans_inside_the_window_are_listed_in_order() {
    let db = setup(&[
        (FROM - DAY, "a"),
        (FROM + 2 * DAY, "b"),
        (FROM, "b"),
        (FROM, "a"),
        (FROM + 30 * DAY, "a"),
    ])
    .await;

    let plans = between(&db, FROM, FROM + 30 * DAY).await;

    let listed: Vec<_> = plans.iter().map(|p| (p.date, p.workout)).collect();
    assert_eq!(listed, [(FROM, A), (FROM, B), (FROM + 2 * DAY, B)]);
}

#[tokio::test]
async fn listed_plans_come_with_their_workout() {
    let db = setup(&[(FROM, "a"), (FROM, "b")]).await;

    let plans = between(&db, FROM, FROM + DAY).await;

    let created: Vec<_> = plans
        .iter()
        .map(|p| p.workout_obj.as_ref().map(|w| w.connect_created_time))
        .collect();
    assert_eq!(created, [Some(Some(1_793_401_668)), Some(None)]);
}

/// Registers the devices `watch` and `other` and links workouts to devices as `(workout, device, pending)`.
async fn link(db: &DatabasePool, links: Vec<(&'static str, &'static str, bool)>) {
    db.run_in_transaction(async move |tx| {
        for serial in ["watch", "other", constants::CONNECT_DEVICE_SERIAL] {
            DeviceRepository::insert()
                .or_ignore()
                .item(&mut Device {
                    serial: serial.to_string(),
                    model: "Fenix".to_string(),
                    last_activity_sync: None,
                    last_body_metric_sync: None,
                    usb: serial != constants::CONNECT_DEVICE_SERIAL,
                    connect: serial == constants::CONNECT_DEVICE_SERIAL,
                    strength_workout: true,
                })
                .execute_in(tx)
                .await?;
        }
        for (workout, device, pending) in links {
            WorkoutDeviceRepository::insert()
                .item(&mut WorkoutDevice {
                    workout: id(workout),
                    device: device.to_string(),
                    pending,
                })
                .execute_in(tx)
                .await?;
        }
        Ok(())
    })
    .await
    .unwrap();
}

async fn pending_links(db: &DatabasePool) -> Vec<(u16, String)> {
    let mut links: Vec<(u16, String)> = WorkoutDeviceRepository::select()
        .fetch(db)
        .await
        .unwrap()
        .into_iter()
        .filter(|l| l.pending)
        .map(|l| (l.workout, l.device))
        .collect();
    links.sort();
    links
}

#[tokio::test]
async fn only_workouts_sent_to_the_device_count_as_on_it() {
    let db = setup(&[]).await;
    link(
        &db,
        vec![
            ("a", "watch", false),
            ("b", "watch", true),
            ("b", "other", false),
        ],
    )
    .await;

    let on_device = db
        .run_in_connection(async |conn| Ok(WorkoutDevice::get_sent(conn, "watch").await?))
        .await
        .unwrap();

    assert_eq!(on_device, [A].into());
}

#[tokio::test]
async fn new_connect_creation_time_queues_the_workout_for_its_devices() {
    let db = setup(&[]).await;
    link(
        &db,
        vec![
            ("b", "watch", false),
            ("b", "other", false),
            ("b", constants::CONNECT_DEVICE_SERIAL, false),
            ("a", "watch", false),
        ],
    )
    .await;

    let changed = db
        .run_in_transaction(async |tx| {
            Ok(Workout::set_connect_created_times(
                tx,
                &[
                    ("1746213483".to_string(), 1_793_500_000),
                    ("1746212841".to_string(), 1_793_401_668),
                ],
            )
            .await?)
        })
        .await
        .unwrap();

    assert_eq!(changed, 1);
    assert_eq!(
        pending_links(&db).await,
        [(B, "other".to_string()), (B, "watch".to_string())]
    );
}

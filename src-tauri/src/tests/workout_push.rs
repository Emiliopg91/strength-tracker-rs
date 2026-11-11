use rusqlite_orm::{dao::Repository, database::DatabasePool};

use crate::{
    dao::{
        device::{Device, DeviceRepository},
        workout::{Workout, WorkoutRepository},
        workout_device::{WorkoutDevice, WorkoutDeviceRepository},
    },
    logic::{
        connect::queries::{pending_connect_workouts, set_connect_pending},
        devices::queries::pending_connect_devices,
    },
    tests::common,
    utils::constants,
};

const LINKED: &str = "linked";
const LINKED_SENT: &str = "linked_sent";
const USB_ONLY: &str = "usb_only";

async fn setup() -> DatabasePool {
    let db = common::test_db().await;
    db.run_in_transaction(move |tx| {
        for name in ["a", "b"] {
            WorkoutRepository::insert()
                .item(&mut Workout {
                    name: name.to_string(),
                    enabled: true,
                    notes: None,
                    steps: Vec::new(),
                    connect_id: None,
                })
                .execute_in(tx)?;
        }

        for (serial, connect) in [(LINKED, true), (LINKED_SENT, true), (USB_ONLY, false)] {
            DeviceRepository::insert()
                .item(&mut Device {
                    serial: serial.to_string(),
                    model: "model".to_string(),
                    last_activity_sync: None,
                    last_body_metric_sync: None,
                    usb: !connect,
                    connect,
                    strength_workout: true,
                })
                .execute_in(tx)?;
        }

        for (workout, device, pending) in [
            ("a", LINKED, true),
            ("a", LINKED_SENT, false),
            ("a", USB_ONLY, true),
            ("b", LINKED, true),
        ] {
            WorkoutDeviceRepository::insert()
                .item(&mut WorkoutDevice {
                    workout: workout.to_string(),
                    device: device.to_string(),
                    pending,
                })
                .execute_in(tx)?;
        }
        Ok(())
    })
    .await
    .unwrap();
    db
}

#[tokio::test]
async fn only_linked_devices_still_waiting_for_the_workout_are_returned() {
    let db = setup().await;

    let serials = db
        .run_in_connection(move |conn| Ok(pending_connect_devices(conn, "a")?))
        .await
        .unwrap();

    assert_eq!(serials, [LINKED]);
}

#[tokio::test]
async fn workout_without_entries_has_no_devices() {
    let db = setup().await;

    let serials = db
        .run_in_connection(move |conn| Ok(pending_connect_devices(conn, "c")?))
        .await
        .unwrap();

    assert!(serials.is_empty());
}

async fn connect_entry(db: &DatabasePool, workout: &str) -> Option<bool> {
    WorkoutDeviceRepository::select_by_id(db, workout, constants::CONNECT_DEVICE_SERIAL)
        .await
        .unwrap()
        .map(|e| e.pending)
}

#[tokio::test]
async fn connect_pending_is_created_and_cleared() {
    let db = setup().await;
    let workouts = ["a".to_string(), "b".to_string()];

    let all = workouts.clone();
    db.run_in_transaction(move |tx| Ok(set_connect_pending(tx, &all, true)?))
        .await
        .unwrap();
    assert_eq!(connect_entry(&db, "a").await, Some(true));
    assert_eq!(connect_entry(&db, "b").await, Some(true));

    db.run_in_transaction(move |tx| Ok(set_connect_pending(tx, &workouts[..1], false)?))
        .await
        .unwrap();
    assert_eq!(connect_entry(&db, "a").await, Some(false));
    assert_eq!(connect_entry(&db, "b").await, Some(true));
}

#[tokio::test]
async fn connect_pending_is_not_a_linked_device() {
    let db = setup().await;

    db.run_in_transaction(move |tx| Ok(set_connect_pending(tx, &["b".to_string()], true)?))
        .await
        .unwrap();
    let serials = db
        .run_in_connection(move |conn| Ok(pending_connect_devices(conn, "b")?))
        .await
        .unwrap();

    assert_eq!(serials, [LINKED]);
}

#[tokio::test]
async fn only_workouts_pending_for_connect_are_listed() {
    let db = setup().await;

    db.run_in_transaction(move |tx| {
        set_connect_pending(tx, &["a".to_string()], false)?;
        set_connect_pending(tx, &["b".to_string()], true)?;
        Ok(())
    })
    .await
    .unwrap();
    let pending = db
        .run_in_connection(move |conn| Ok(pending_connect_workouts(conn)?))
        .await
        .unwrap();

    // Workouts pending only for real devices are not listed
    assert_eq!(pending, ["b"]);
}

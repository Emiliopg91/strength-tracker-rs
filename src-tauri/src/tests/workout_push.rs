use rusqlite_orm::{dao::Repository, database::DatabasePool};

use crate::{
    dao::{
        device::{Device, DeviceRepository},
        workout::{Workout, WorkoutRepository},
        workout_device::{WorkoutDevice, WorkoutDeviceRepository},
    },
    tests::common,
    utils::constants,
};

const LINKED: &str = "linked";
const LINKED_SENT: &str = "linked_sent";
const USB_ONLY: &str = "usb_only";

/// Ids of the workouts "a" and "b", inserted in that order on an empty database
const A: u16 = 1;
const B: u16 = 2;

async fn setup() -> DatabasePool {
    let db = common::test_db().await;
    db.run_in_transaction(async move |tx| {
        for name in ["a", "b"] {
            WorkoutRepository::insert()
                .item(&mut Workout {
                    id: 0,
                    name: name.to_string(),
                    enabled: true,
                    notes: None,
                    steps: Vec::new(),
                    connect_id: None,
                    connect_created_time: None,
                    progression: true,
                })
                .execute_in(tx)
                .await?;
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
                .execute_in(tx)
                .await?;
        }

        for (workout, device, pending) in [
            (A, LINKED, true),
            (A, LINKED_SENT, false),
            (A, USB_ONLY, true),
            (B, LINKED, true),
        ] {
            WorkoutDeviceRepository::insert()
                .item(&mut WorkoutDevice {
                    workout,
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
    db
}

#[tokio::test]
async fn only_linked_devices_still_waiting_for_the_workout_are_returned() {
    let db = setup().await;

    let serials = db
        .run_in_connection(async move |conn| {
            Ok(WorkoutDevice::get_pending_connect_devices(conn, A).await?)
        })
        .await
        .unwrap();

    assert_eq!(serials, [LINKED]);
}

#[tokio::test]
async fn workout_without_entries_has_no_devices() {
    let db = setup().await;

    let serials = db
        .run_in_connection(async move |conn| {
            Ok(WorkoutDevice::get_pending_connect_devices(conn, 3).await?)
        })
        .await
        .unwrap();

    assert_eq!(serials, Vec::<String>::new());
}

async fn connect_entry(db: &DatabasePool, workout: u16) -> Option<bool> {
    WorkoutDeviceRepository::select_by_id(db, workout, constants::CONNECT_DEVICE_SERIAL)
        .await
        .unwrap()
        .map(|e| e.pending)
}

#[tokio::test]
async fn connect_pending_is_created_and_cleared() {
    let db = setup().await;
    let workouts = [A, B];

    let all = workouts;
    db.run_in_transaction(async move |tx| {
        Ok(WorkoutDevice::set_connect_pending(tx, &all, true).await?)
    })
    .await
    .unwrap();
    assert_eq!(connect_entry(&db, A).await, Some(true));
    assert_eq!(connect_entry(&db, B).await, Some(true));

    db.run_in_transaction(async move |tx| {
        Ok(WorkoutDevice::set_connect_pending(tx, &workouts[..1], false).await?)
    })
    .await
    .unwrap();
    assert_eq!(connect_entry(&db, A).await, Some(false));
    assert_eq!(connect_entry(&db, B).await, Some(true));
}

#[tokio::test]
async fn connect_pending_is_not_a_linked_device() {
    let db = setup().await;

    db.run_in_transaction(async move |tx| {
        Ok(WorkoutDevice::set_connect_pending(tx, &[B], true).await?)
    })
    .await
    .unwrap();
    let serials = db
        .run_in_connection(async move |conn| {
            Ok(WorkoutDevice::get_pending_connect_devices(conn, B).await?)
        })
        .await
        .unwrap();

    assert_eq!(serials, [LINKED]);
}

#[tokio::test]
async fn only_workouts_pending_for_connect_are_listed() {
    let db = setup().await;

    db.run_in_transaction(async move |tx| {
        WorkoutDevice::set_connect_pending(tx, &[A], false).await?;
        WorkoutDevice::set_connect_pending(tx, &[B], true).await?;
        Ok(())
    })
    .await
    .unwrap();
    let pending = db
        .run_in_connection(async move |conn| {
            Ok(WorkoutDevice::get_pending(conn, constants::CONNECT_DEVICE_SERIAL, None).await?)
        })
        .await
        .unwrap();

    // Workouts pending only for real devices are not listed
    assert_eq!(pending, [(B, "b".to_string())]);
}

use rusqlite_orm::{dao::Repository, database::DatabasePool};

use crate::{
    dao::{
        device::{Device, DeviceRepository},
        workout::{Workout, WorkoutRepository},
        workout_device::{WorkoutDevice, WorkoutDeviceRepository},
    },
    logic::devices::pending_connect_devices,
    tests::common,
};

const LINKED: &str = "linked";
const LINKED_SENT: &str = "linked_sent";
const USB_ONLY: &str = "usb_only";

fn setup() -> DatabasePool {
    let db = common::test_db();
    db.run_in_transaction(|tx| {
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
    .unwrap();
    db
}

#[test]
fn only_linked_devices_still_waiting_for_the_workout_are_returned() {
    let db = setup();

    let serials = db
        .run_in_connection(|conn| Ok(pending_connect_devices(conn, "a")?))
        .unwrap();

    assert_eq!(serials, [LINKED]);
}

#[test]
fn workout_without_entries_has_no_devices() {
    let db = setup();

    let serials = db
        .run_in_connection(|conn| Ok(pending_connect_devices(conn, "c")?))
        .unwrap();

    assert!(serials.is_empty());
}

use rusqlite_orm::{dao::Repository, database::DatabasePool};

use crate::{
    dao::device::{Device, DeviceRepository},
    tests::common,
};

fn device(serial: &str, model: &str, connect: bool) -> Device {
    Device {
        serial: serial.to_string(),
        model: model.to_string(),
        last_activity_sync: None,
        last_body_metric_sync: None,
        usb: false,
        connect,
        strength_workout: true,
    }
}

async fn stored(db: &DatabasePool, serial: &str) -> Option<(String, bool)> {
    DeviceRepository::select_by_id(db, serial)
        .await
        .unwrap()
        .map(|d| (d.model, d.connect))
}

#[tokio::test]
async fn sync_connect_adds_updates_and_unflags_devices() {
    let db = common::test_db().await;
    db.run_in_transaction(async |tx| {
        for mut dao in [
            device("unchanged", "Watch", true),
            device("renamed", "Old", true),
            device("usb_only", "Watch", false),
            device("gone", "Watch", true),
        ] {
            DeviceRepository::insert()
                .item(&mut dao)
                .execute_in(tx)
                .await?;
        }
        Ok(())
    })
    .await
    .unwrap();

    let listed = vec![
        device("unchanged", "Watch", true),
        device("renamed", "New", true),
        device("usb_only", "Watch", true),
        device("new", "Watch", true),
    ];
    let discovered = db
        .run_in_transaction(async move |tx| Ok(Device::sync_connect(tx, &listed).await?))
        .await
        .unwrap();

    assert_eq!(discovered.len(), 4);
    assert_eq!(stored(&db, "unchanged").await, Some(("Watch".into(), true)));
    assert_eq!(stored(&db, "renamed").await, Some(("New".into(), true)));
    assert_eq!(stored(&db, "usb_only").await, Some(("Watch".into(), true)));
    assert_eq!(stored(&db, "new").await, Some(("Watch".into(), true)));
    assert_eq!(stored(&db, "gone").await, Some(("Watch".into(), false)));
}

#[tokio::test]
async fn sync_connect_with_nothing_listed_unflags_every_device() {
    let db = common::test_db().await;
    db.run_in_transaction(async |tx| {
        let mut dao = device("gone", "Watch", true);
        DeviceRepository::insert()
            .item(&mut dao)
            .execute_in(tx)
            .await?;
        Ok(())
    })
    .await
    .unwrap();

    let discovered = db
        .run_in_transaction(async |tx| Ok(Device::sync_connect(tx, &[]).await?))
        .await
        .unwrap();

    assert!(discovered.is_empty());
    assert_eq!(stored(&db, "gone").await, Some(("Watch".into(), false)));
}

use rusqlite_orm::{dao::Repository, database::DatabasePool};

use crate::{
    dao::{
        device::{Device, DeviceRepository},
        session::{Session, SessionRepository},
        workout::{Workout, WorkoutRepository},
    },
    fit::{FitFile, parser::FitParser},
    logic::sessions::helpers::{ParsedFiles, import_file_list, parse_files},
    tests::common,
};

/// Serial of the watch that recorded the fixtures, `3000000001` in hexadecimal.
const FIXTURE_SERIAL: &str = "0000b2d05e01";

fn fixture_path(name: &str) -> String {
    format!(
        "{}/src/tests/fixtures/{name}.fit",
        env!("CARGO_MANIFEST_DIR")
    )
}

fn parse_fixture(name: &str) -> Session {
    FitParser::from_file(fixture_path(name))
        .and_then(Session::try_from)
        .unwrap_or_else(|e| panic!("{name}: {e:?}"))
}

async fn register_device(db: &DatabasePool, serial: &str) {
    let serial = serial.to_string();
    db.run_in_transaction(async move |tx| {
        DeviceRepository::insert()
            .item(&mut Device {
                serial: serial.clone(),
                model: "MODEL".to_string(),
                last_activity_sync: None,
                last_body_metric_sync: None,
                usb: true,
                connect: false,
                strength_workout: true,
            })
            .execute_in(tx)
            .await?;
        Ok(())
    })
    .await
    .unwrap();
}

/// Imports the fixture and returns the device stored in its session.
async fn import(db: &DatabasePool, fixture: &str) -> Option<String> {
    let files = [FitFile::read(fixture_path(fixture)).unwrap()];
    let imported = db
        .run_in_transaction(async move |tx| Ok(import_file_list(tx, parse_files(&files)).await?))
        .await
        .unwrap();
    assert_eq!(imported.sessions.len(), 1);

    SessionRepository::select_by_id(db, imported.sessions[0])
        .await
        .unwrap()
        .unwrap()
        .device
}

#[test]
fn parser_reads_the_serial_of_the_creator_device() {
    // In these files the last device_info messages belong to sensors with serial 0
    for fixture in ["strength_1", "walk_oslo", "indoor_biking"] {
        assert_eq!(
            parse_fixture(fixture).device.as_deref(),
            Some(FIXTURE_SERIAL),
            "{fixture}"
        );
    }
}

#[tokio::test]
async fn registered_device_is_kept() {
    let db = common::test_db().await;
    register_device(&db, FIXTURE_SERIAL).await;

    assert_eq!(
        import(&db, "strength_1").await.as_deref(),
        Some(FIXTURE_SERIAL)
    );
}

#[tokio::test]
async fn unknown_device_is_cleared() {
    let db = common::test_db().await;
    register_device(&db, "another").await;

    assert_eq!(import(&db, "strength_1").await, None);
}

/// Imports the given files and returns the Connect id stored in the single session created.
async fn import_connect_id(db: &DatabasePool, file: FitFile) -> Option<String> {
    let files = [file];
    let imported = db
        .run_in_transaction(async move |tx| Ok(import_file_list(tx, parse_files(&files)).await?))
        .await
        .unwrap();
    assert_eq!(imported.sessions.len(), 1);

    SessionRepository::select_by_id(db, imported.sessions[0])
        .await
        .unwrap()
        .unwrap()
        .connect_id
}

#[tokio::test]
async fn connect_activity_id_is_stored() {
    let db = common::test_db().await;
    let file = FitFile::read(fixture_path("strength_1"))
        .unwrap()
        .with_activity_id(25017923159);

    assert_eq!(
        import_connect_id(&db, file).await.as_deref(),
        Some("25017923159")
    );
}

#[tokio::test]
async fn local_file_has_no_connect_id() {
    let db = common::test_db().await;
    let file = FitFile::read(fixture_path("strength_1")).unwrap();

    assert_eq!(import_connect_id(&db, file).await, None);
}

fn session_of_workout(date: u32, notes: &str) -> Session {
    let mut session = common::session(date);
    session.workout = Some("Legs".to_string());
    session.workout_obj = Some(Workout {
        name: "Legs".to_string(),
        enabled: true,
        notes: Some(notes.to_string()),
        steps: Vec::new(),
        connect_id: None,
        connect_created_time: None,
    });
    session
}

#[tokio::test]
async fn workout_keeps_the_notes_of_its_latest_session() {
    let db = common::test_db().await;
    // Oldest first, as `parse_files` hands them
    let parsed = ParsedFiles {
        sessions: vec![
            session_of_workout(1, "Oldest"),
            session_of_workout(2, "Middle"),
            session_of_workout(3, "Latest"),
        ],
        failed: Vec::new(),
    };

    let imported = db
        .run_in_transaction(async move |tx| Ok(import_file_list(tx, parsed).await?))
        .await
        .unwrap();

    assert_eq!(imported.sessions, [1, 2, 3]);
    let workout = WorkoutRepository::select_by_id(&db, "Legs")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(workout.notes.as_deref(), Some("Latest"));
}

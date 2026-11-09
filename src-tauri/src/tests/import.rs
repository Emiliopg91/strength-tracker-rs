use rusqlite_orm::{dao::Repository, database::DatabasePool};

use crate::{
    dao::{
        device::{Device, DeviceRepository},
        session::{Session, SessionRepository},
    },
    fit::{FitFile, parser::FitParser},
    logic::sessions::import_file_list,
    tests::common,
    utils::translations::Languages,
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

fn register_device(db: &DatabasePool, serial: &str) {
    db.run_in_transaction(|tx| {
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
        Ok(())
    })
    .unwrap();
}

/// Imports the fixture and returns the device stored in its session.
fn import(db: &DatabasePool, fixture: &str) -> Option<String> {
    let files = [FitFile::read(fixture_path(fixture)).unwrap()];
    let imported = db
        .run_in_transaction(|tx| Ok(import_file_list(tx, &files, Languages::English)?))
        .unwrap();
    assert_eq!(imported.sessions.len(), 1);

    SessionRepository::select_by_id(db, imported.sessions[0])
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

#[test]
fn registered_device_is_kept() {
    let db = common::test_db();
    register_device(&db, FIXTURE_SERIAL);

    assert_eq!(import(&db, "strength_1").as_deref(), Some(FIXTURE_SERIAL));
}

#[test]
fn unknown_device_is_cleared() {
    let db = common::test_db();
    register_device(&db, "another");

    assert_eq!(import(&db, "strength_1"), None);
}

/// Imports the given files and returns the Connect id stored in the single session created.
fn import_connect_id(db: &DatabasePool, file: FitFile) -> Option<String> {
    let files = [file];
    let imported = db
        .run_in_transaction(|tx| Ok(import_file_list(tx, &files, Languages::English)?))
        .unwrap();
    assert_eq!(imported.sessions.len(), 1);

    SessionRepository::select_by_id(db, imported.sessions[0])
        .unwrap()
        .unwrap()
        .connect_id
}

#[test]
fn connect_activity_id_is_stored() {
    let db = common::test_db();
    let file = FitFile::read(fixture_path("strength_1"))
        .unwrap()
        .with_activity_id(25017923159);

    assert_eq!(
        import_connect_id(&db, file).as_deref(),
        Some("25017923159")
    );
}

#[test]
fn local_file_has_no_connect_id() {
    let db = common::test_db();
    let file = FitFile::read(fixture_path("strength_1")).unwrap();

    assert_eq!(import_connect_id(&db, file), None);
}

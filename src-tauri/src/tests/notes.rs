use rusqlite_orm::database::DatabasePool;

use crate::{
    dao::session_data::{SessionData, SessionDataRepository},
    logic::sessions::update_session_notes,
    tests::common,
};

const WITH_DATA: u32 = 1;
const WITHOUT_DATA: u32 = 2;
const OTHER: u32 = 3;

/// Inserts a session with an additional data row (`WITH_DATA`, `OTHER`) and one without it (`WITHOUT_DATA`).
fn setup() -> DatabasePool {
    let db = common::test_db();
    db.run_in_transaction(|tx| {
        for date in [WITH_DATA, OTHER] {
            let mut session = common::session(date);
            let mut add_data = common::additional_data(date);
            add_data.notes = Some(format!("Notes {date}"));
            session.data = Some(add_data);
            common::insert_session(tx, session);
        }
        common::insert_session(tx, common::session(WITHOUT_DATA));
        Ok(())
    })
    .unwrap();
    db
}

fn save_notes(db: &DatabasePool, session: u32, notes: Option<&str>) {
    db.run_in_transaction(|tx| Ok(update_session_notes(tx, session, notes)?))
        .unwrap();
}

fn additional_data(db: &DatabasePool, session: u32) -> Option<SessionData> {
    SessionDataRepository::select_by_id(db, session).unwrap()
}

fn notes(db: &DatabasePool, session: u32) -> Option<String> {
    additional_data(db, session).and_then(|a| a.notes)
}

#[test]
fn updating_notes_does_not_touch_other_sessions() {
    let db = setup();

    save_notes(&db, WITH_DATA, Some("Updated"));

    assert_eq!(notes(&db, WITH_DATA).as_deref(), Some("Updated"));
    assert_eq!(notes(&db, OTHER).as_deref(), Some("Notes 3"));
}

#[test]
fn clearing_notes_does_not_touch_other_sessions() {
    let db = setup();

    save_notes(&db, WITH_DATA, None);

    assert_eq!(notes(&db, WITH_DATA), None);
    assert_eq!(notes(&db, OTHER).as_deref(), Some("Notes 3"));
}

#[test]
fn notes_on_session_without_additional_data_are_stored() {
    let db = setup();

    save_notes(&db, WITHOUT_DATA, Some("New notes"));

    assert_eq!(notes(&db, WITHOUT_DATA).as_deref(), Some("New notes"));
    assert_eq!(notes(&db, WITH_DATA).as_deref(), Some("Notes 1"));
    assert_eq!(notes(&db, OTHER).as_deref(), Some("Notes 3"));
}

#[test]
fn blank_notes_are_stored_as_null() {
    let db = setup();

    save_notes(&db, WITH_DATA, Some("   \n\t"));

    assert!(additional_data(&db, WITH_DATA).is_some());
    assert_eq!(notes(&db, WITH_DATA), None);
}

#[test]
fn blank_notes_on_session_without_additional_data_create_no_row() {
    let db = setup();

    save_notes(&db, WITHOUT_DATA, Some("  "));
    save_notes(&db, WITHOUT_DATA, None);

    assert!(additional_data(&db, WITHOUT_DATA).is_none());
}

#[test]
fn updating_notes_keeps_other_additional_data() {
    let db = setup();
    let mut before = additional_data(&db, WITH_DATA).unwrap();
    before.heart_rates = Some(vec![120, 130, 140]);
    before.distance = Some(1234.5);
    db.run_in_transaction(|tx| Ok(before.update_by_id_in(tx)?))
        .unwrap();

    save_notes(&db, WITH_DATA, Some("Updated"));

    let after = additional_data(&db, WITH_DATA).unwrap();
    assert_eq!(after.heart_rates, before.heart_rates);
    assert_eq!(after.distance, before.distance);
}

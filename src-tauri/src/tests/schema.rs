use rusqlite_orm::{dao::Repository, ddls};

use crate::{
    dao::{
        additional_data::AdditionalDataRepository, lap::LapRepository, session::SessionRepository,
        set::SetRepository, workout::WorkoutRepository,
    },
    schema_ddls,
    tests::common,
};

#[test]
fn schema_version_matches_last_ddl() {
    let db = common::test_db();
    let last = ddls!("../resources/ddl")
        .iter()
        .map(|d| d.version)
        .max()
        .unwrap();

    let version: u16 = db
        .run_in_connection(|conn| Ok(conn.pragma_query_value(None, "user_version", |r| r.get(0))?))
        .unwrap();

    assert_eq!(version, last);
}

#[test]
fn schema_ddls_are_sorted_with_hooks_on_their_migration() {
    let ddls = schema_ddls();

    let versions = ddls.iter().map(|d| d.version).collect::<Vec<_>>();
    assert_eq!(versions, (1..=versions.len() as u16).collect::<Vec<_>>());

    let with_hooks = ddls
        .iter()
        .filter(|d| d.update_fn.is_some())
        .map(|d| d.version)
        .collect::<Vec<_>>();
    assert!(with_hooks.is_empty());
}

#[test]
fn schema_creation_is_idempotent() {
    let db = common::test_db();
    db.create_schema(&ddls!("../resources/ddl")).unwrap();
}

#[test]
fn session_roundtrip_with_relationships() {
    let db = common::test_db();

    let mut session = common::session(1_700_000_000);
    session.workout = Some("Leg day".to_string());
    session.sets = vec![common::set(1_700_000_000, 0, (28, 0), 5, 100.0)];
    session.laps = vec![common::lap(1_700_000_000, 0, Some((1, 2)))];
    let mut data = common::additional_data(1_700_000_000);
    data.notes = Some("Felt strong".to_string());
    data.distance = Some(1_000.0);
    session.additional_data = Some(data);

    db.run_in_transaction(|tx| {
        WorkoutRepository::insert()
            .item(&mut crate::dao::workout::Workout {
                name: "Leg day".to_string(),
                enabled: true,
                notes: Some("".to_string()),
                steps: Vec::new(),
            })
            .execute_in(tx)?;
        common::insert_session(tx, session.clone());
        Ok(())
    })
    .unwrap();

    let stored = db
        .run_in_connection(|conn| {
            let mut s = SessionRepository::select_by_id_in(conn, 1_700_000_000_u32)?.unwrap();
            s.fetch_sets_relationship_in(conn)?;
            s.fetch_laps_relationship_in(conn)?;
            s.fetch_additional_data_relationship_in(conn)?;
            s.fetch_workout_obj_relationship_in(conn)?;
            Ok(s)
        })
        .unwrap();

    assert_eq!(stored.name, session.name);
    assert_eq!(stored.sets.len(), 1);
    assert_eq!(stored.sets[0].weight, 100.0);
    assert_eq!(stored.laps.len(), 1);
    assert_eq!(stored.laps[0].start_latitude, Some(1));
    let data = stored.additional_data.unwrap();
    assert_eq!(data.notes.as_deref(), Some("Felt strong"));
    assert_eq!(data.distance, Some(1_000.0));
    assert!(stored.workout_obj.unwrap().enabled);
}

#[test]
fn deleting_a_session_cascades() {
    let db = common::test_db();

    let mut session = common::session(1);
    session.sets = vec![common::set(1, 0, (28, 0), 5, 100.0)];
    session.laps = vec![common::lap(1, 0, None)];
    session.additional_data = Some(common::additional_data(1));

    db.run_in_transaction(|tx| {
        common::insert_session(tx, session.clone());
        session.delete_by_id_in(tx)?;
        Ok(())
    })
    .unwrap();

    let (sets, laps, data) = db
        .run_in_connection(|conn| {
            Ok((
                SetRepository::select().fetch_in(conn)?.len(),
                LapRepository::select().fetch_in(conn)?.len(),
                AdditionalDataRepository::select().fetch_in(conn)?.len(),
            ))
        })
        .unwrap();

    assert_eq!((sets, laps, data), (0, 0, 0));
}

#[test]
fn set_requires_known_exercise() {
    let db = common::test_db();

    let mut session = common::session(1);
    session.sets = vec![common::set(1, 0, (9999, 9999), 5, 100.0)];

    let res = db.run_in_transaction(|tx| {
        let mut sets = std::mem::take(&mut session.sets);
        SessionRepository::insert()
            .item(&mut session.clone())
            .execute_in(tx)?;
        SetRepository::insert().item(&mut sets[0]).execute_in(tx)?;
        Ok(())
    });

    assert!(res.is_err());
}

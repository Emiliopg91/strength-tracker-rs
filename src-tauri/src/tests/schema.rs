use rusqlite_orm::{dao::Repository, database::builder::DatabaseConnectionBuilder, ddls};

use crate::{
    dao::{
        additional_data::AdditionalDataRepository, lap::LapRepository, session::SessionRepository,
        session_step::SessionStepRepository, workout::WorkoutRepository, workout_step::StepType,
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

    let versions: Vec<_> = ddls.iter().map(|d| d.version).collect();
    assert_eq!(versions, (1..=versions.len() as u16).collect::<Vec<_>>());

    let with_hooks: Vec<_> = ddls
        .iter()
        .filter(|d| d.update_fn.is_some())
        .map(|d| d.version)
        .collect();
    assert_eq!(with_hooks, [21]);
}

#[test]
fn migration_26_turns_rest_flag_into_kind() {
    let db = DatabaseConnectionBuilder::default()
        .enable_foreign_keys()
        .build("test")
        .unwrap();
    let before: Vec<_> = schema_ddls()
        .into_iter()
        .filter(|d| d.version < 26)
        .collect();
    db.create_schema(&before).unwrap();

    db.run_in_transaction(|tx| {
        common::insert_session(tx, common::session(1));
        tx.execute_batch(
            "INSERT INTO SESSION_STEP (session, idx, exercise_category, exercise_id, reps, weight, pr, rest)
             VALUES (1, 0, 0, 1, 10, 50, 0, 0), (1, 1, NULL, NULL, 0, 0, 0, 1);",
        )?;
        Ok(())
    })
    .unwrap();

    db.create_schema(&schema_ddls()).unwrap();

    let mut steps = db
        .run_in_connection(|conn| Ok(SessionStepRepository::select().fetch_in(conn)?))
        .unwrap();
    steps.sort_by_key(|s| s.idx);
    let kinds: Vec<_> = steps.into_iter().map(|s| s.kind).collect();
    assert_eq!(kinds, [StepType::Exercise, StepType::Rest]);

    let has_rest: bool = db
        .run_in_connection(|conn| {
            Ok(conn
                .prepare("SELECT 1 FROM pragma_table_info('SESSION_STEP') WHERE name = 'rest'")?
                .exists([])?)
        })
        .unwrap();
    assert!(!has_rest);
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
    session.steps = vec![common::set(1_700_000_000, 0, (28, 0), 5, 100.0)];
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
                connect_id: None,
            })
            .execute_in(tx)?;
        common::insert_session(tx, session.clone());
        Ok(())
    })
    .unwrap();

    let stored = db
        .run_in_connection(|conn| {
            let mut s = SessionRepository::select_by_id_in(conn, 1_700_000_000_u32)?.unwrap();
            s.fetch_steps_relationship_in(conn)?;
            s.fetch_laps_relationship_in(conn)?;
            s.fetch_additional_data_relationship_in(conn)?;
            s.fetch_workout_obj_relationship_in(conn)?;
            Ok(s)
        })
        .unwrap();

    assert_eq!(stored.name, session.name);
    assert_eq!(stored.steps.len(), 1);
    assert_eq!(stored.steps[0].weight, 100.0);
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
    session.steps = vec![common::set(1, 0, (28, 0), 5, 100.0)];
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
                SessionStepRepository::select().fetch_in(conn)?.len(),
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
    session.steps = vec![common::set(1, 0, (9999, 9999), 5, 100.0)];

    let res = db.run_in_transaction(|tx| {
        let mut sets = std::mem::take(&mut session.steps);
        SessionRepository::insert()
            .item(&mut session.clone())
            .execute_in(tx)?;
        SessionStepRepository::insert()
            .item(&mut sets[0])
            .execute_in(tx)?;
        Ok(())
    });

    assert!(res.is_err());
}

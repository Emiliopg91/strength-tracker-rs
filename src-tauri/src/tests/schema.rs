use rusqlite_orm::{dao::Repository, database::builder::DatabaseConnectionBuilder, ddls};

use crate::{
    dao::{
        session::SessionRepository, session_data::SessionDataRepository,
        session_lap::SessionLapRepository, session_step::SessionStepRepository,
        workout::WorkoutRepository, workout_device::WorkoutDeviceRepository,
        workout_step::StepType,
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
        insert_legacy_session(tx, 1)?;
        tx.execute_batch(
            "INSERT INTO SESSION_STEP (session, idx, exercise_category, exercise_id, reps, weight, pr, rest)
             VALUES (1, 0, 0, 1, 10, 50, 0, 0), (1, 1, NULL, NULL, 0, 0, 0, 1);",
        )?;
        Ok(())
    })
    .unwrap();

    db.create_schema(&schema_ddls()).unwrap();

    let mut steps = SessionStepRepository::select().fetch(&db).unwrap();
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
    session.data = Some(data);

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
            s.fetch_data_relationship_in(conn)?;
            s.fetch_workout_obj_relationship_in(conn)?;
            Ok(s)
        })
        .unwrap();

    assert_eq!(stored.name, session.name);
    assert_eq!(stored.steps.len(), 1);
    assert_eq!(stored.steps[0].weight, Some(100.0));
    assert_eq!(stored.laps.len(), 1);
    assert_eq!(stored.laps[0].start_latitude, Some(1));
    let data = stored.data.unwrap();
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
    session.data = Some(common::additional_data(1));

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
                SessionLapRepository::select().fetch_in(conn)?.len(),
                SessionDataRepository::select().fetch_in(conn)?.len(),
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

#[test]
fn migration_27_nulls_reps_and_weight_only_on_rest_steps() {
    let db = DatabaseConnectionBuilder::default()
        .enable_foreign_keys()
        .build("test")
        .unwrap();
    let before: Vec<_> = schema_ddls()
        .into_iter()
        .filter(|d| d.version < 27)
        .collect();
    db.create_schema(&before).unwrap();

    db.run_in_transaction(|tx| {
        insert_legacy_session(tx, 1)?;
        tx.execute_batch(
            "INSERT INTO SESSION_STEP (session, idx, exercise_category, exercise_id, reps, weight, pr, duration, kind)
             VALUES (1, 0, 0, 1, 10, 50, 1, 60, 'Exercise'),
                    (1, 1, NULL, NULL, 0, 0, 0, 90, 'Rest'),
                    (1, 2, NULL, NULL, 0, 0, 0, 30, 'Warmup');",
        )?;
        Ok(())
    })
    .unwrap();

    db.create_schema(&schema_ddls()).unwrap();

    let mut steps = SessionStepRepository::select().fetch(&db).unwrap();
    steps.sort_by_key(|s| s.idx);

    let values: Vec<_> = steps
        .iter()
        .map(|s| (s.reps, s.weight, s.pr, s.duration, s.kind.clone()))
        .collect();
    assert_eq!(
        values,
        [
            (Some(10), Some(50.0), true, Some(60), StepType::Exercise),
            (None, None, false, Some(90), StepType::Rest),
            (Some(0), Some(0.0), false, Some(30), StepType::Warmup),
        ]
    );

    let indexes: i64 = db
        .run_in_connection(|conn| {
            Ok(conn.query_row(
                "SELECT count(*) FROM sqlite_master WHERE type = 'index' AND tbl_name = 'SESSION_STEP' AND sql IS NOT NULL",
                [],
                |r| r.get(0),
            )?)
        })
        .unwrap();
    assert_eq!(indexes, 4);
}

#[test]
fn migration_27_keeps_the_children_of_rebuilt_tables() {
    let db = DatabaseConnectionBuilder::default()
        .enable_foreign_keys()
        .build("test")
        .unwrap();
    let before: Vec<_> = schema_ddls()
        .into_iter()
        .filter(|d| d.version < 27)
        .collect();
    db.create_schema(&before).unwrap();

    db.run_in_transaction(|tx| {
        insert_legacy_session(tx, 1)?;
        tx.execute_batch(
            "INSERT INTO LAP (session, idx) VALUES (1, 0);
             INSERT INTO ADDITIONAL_DATA (session, max_hr) VALUES (1, 190);
             INSERT INTO SESSION_STEP (session, idx, exercise_category, exercise_id, reps, weight, pr, duration, kind)
             VALUES (1, 0, 0, 1, 10, 50, 0, 60, 'Exercise');
             INSERT INTO WORKOUT (name) VALUES ('Push');
             INSERT INTO WORKOUT_STEP (workout, idx, kind, exercise_category, exercise_id) VALUES ('Push', 0, 'Exercise', 0, 1);",
        )?;
        Ok(())
    })
    .unwrap();
    let count = |table: &str| -> i64 {
        db.run_in_connection(|conn| {
            Ok(conn.query_row(&format!("SELECT count(*) FROM {table}"), [], |r| r.get(0))?)
        })
        .unwrap()
    };
    let tables = [
        "SESSION",
        "LAP",
        "ADDITIONAL_DATA",
        "SESSION_STEP",
        "WORKOUT",
        "WORKOUT_STEP",
        "MUSCLE",
        "MUSCLE_EXERCISE",
        "EXERCISE",
        "SUB_SPORT",
    ];
    let expected: Vec<_> = tables.iter().map(|t| count(t)).collect();
    assert!(expected.iter().all(|c| *c > 0));

    // Up to 28: later migrations add catalog rows on purpose
    let up_to_28: Vec<_> = schema_ddls()
        .into_iter()
        .filter(|d| d.version <= 28)
        .collect();
    db.create_schema(&up_to_28).unwrap();

    // Migration 28 renames LAP and ADDITIONAL_DATA
    let renamed = |table: &'static str| match table {
        "LAP" => "SESSION_LAP",
        "ADDITIONAL_DATA" => "SESSION_DATA",
        other => other,
    };
    assert_eq!(
        tables.iter().map(|t| count(renamed(t))).collect::<Vec<_>>(),
        expected
    );
}

#[test]
fn catalogs_in_use_cannot_be_deleted() {
    let db = common::test_db();
    db.run_in_transaction(|tx| {
        let mut session = common::session(1);
        session.steps = vec![common::set(1, 0, (0, 1), 10, 50.0)];
        common::insert_session(tx, session);
        Ok(())
    })
    .unwrap();
    let run = |sql: &str| {
        db.run_in_transaction(|tx| {
            tx.execute_batch(sql)?;
            Ok(())
        })
    };

    assert!(run("DELETE FROM SUB_SPORT WHERE sport = 10 AND id = 20").is_err());
    assert!(run("DELETE FROM EXERCISE WHERE category = 0 AND id = 1").is_err());
}

#[test]
fn migration_27_turns_workout_step_time_into_seconds() {
    let db = DatabaseConnectionBuilder::default()
        .enable_foreign_keys()
        .build("test")
        .unwrap();
    let before: Vec<_> = schema_ddls()
        .into_iter()
        .filter(|d| d.version < 27)
        .collect();
    db.create_schema(&before).unwrap();
    db.run_in_transaction(|tx| {
        tx.execute_batch(
            "INSERT INTO WORKOUT (name) VALUES ('Push');
             INSERT INTO WORKOUT_STEP (workout, idx, kind, time)
             VALUES ('Push', 0, 'Rest', 90000), ('Push', 1, 'Rest', NULL);",
        )?;
        Ok(())
    })
    .unwrap();

    db.create_schema(&schema_ddls()).unwrap();

    let mut steps = db
        .run_in_connection(|conn| {
            let mut stmt = conn.prepare("SELECT time FROM WORKOUT_STEP ORDER BY idx")?;
            let rows = stmt.query_map([], |r| r.get::<_, Option<u32>>(0))?;
            Ok(rows.collect::<Result<Vec<_>, _>>()?)
        })
        .unwrap();
    assert_eq!(steps.remove(0), Some(90));
    assert_eq!(steps.remove(0), None);
}

#[test]
fn migration_28_drops_chart_flag_and_cascades_workout_device() {
    let db = DatabaseConnectionBuilder::default()
        .enable_foreign_keys()
        .build("test")
        .unwrap();
    let before: Vec<_> = schema_ddls()
        .into_iter()
        .filter(|d| d.version < 28)
        .collect();
    db.create_schema(&before).unwrap();

    db.run_in_transaction(|tx| {
        tx.execute_batch(
            "INSERT INTO WORKOUT (name, enabled) VALUES ('a', 1), ('b', 1);
             INSERT INTO WORKOUT_DEVICE (workout, device, pending)
             VALUES ('a', 'connect', 1), ('b', 'connect', 0);",
        )?;
        Ok(())
    })
    .unwrap();

    db.create_schema(&schema_ddls()).unwrap();

    let entries = |db: &rusqlite_orm::database::DatabasePool| {
        let mut entries: Vec<_> = WorkoutDeviceRepository::select()
            .fetch(db)
            .unwrap()
            .into_iter()
            .map(|e| (e.workout, e.pending))
            .collect();
        entries.sort();
        entries
    };
    assert_eq!(
        entries(&db),
        [("a".to_string(), true), ("b".to_string(), false)]
    );

    let has_chart_flag: bool = db
        .run_in_connection(|conn| {
            Ok(conn.query_row(
                "SELECT COUNT(*) > 0 FROM pragma_table_info('SESSION') WHERE name = 'show_exercises_chart'",
                [],
                |r| r.get(0),
            )?)
        })
        .unwrap();
    assert!(!has_chart_flag);

    WorkoutRepository::delete()
        .where_(rusqlite_orm::types::where_clause::Where::Eq(
            crate::dao::workout::entity::columns::NAME,
            "a".into(),
        ))
        .execute(&db)
        .unwrap();
    assert_eq!(entries(&db), [("b".to_string(), false)]);
}

/// Inserts a session with raw SQL, using only the columns that exist before migration 28:
/// the `Session` entity tracks the latest schema, so its inserts fail on older ones.
fn insert_legacy_session(
    tx: &rusqlite_orm::rusqlite::Transaction,
    date: u32,
) -> rusqlite_orm::rusqlite::Result<()> {
    let session = common::session(date);
    tx.execute(
        "INSERT INTO SESSION (date, name, total_elapsed_time, active_time, active_calories, training_load, sport, sub_sport)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        rusqlite_orm::rusqlite::params![
            session.date,
            session.name,
            session.total_elapsed_time,
            session.active_time,
            session.active_calories,
            session.training_load,
            session.sport,
            session.sub_sport,
        ],
    )?;
    Ok(())
}

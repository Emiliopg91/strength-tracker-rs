use rusqlite_orm::{dao::Repository, database::builder::DatabaseConnectionBuilder};

use crate::{
    dao::{
        exercise::{ExerciseRepository, GENERIC_EXERCISE_ID},
        exercise_category::ExerciseCategoryRepository,
        session_step::SessionStepRepository,
        workout_step::WorkoutStepRepository,
    },
    logic::exercises::queries::exercises_catalog,
    schema_ddls,
    tests::common,
};

#[tokio::test]
async fn migration_29_moves_hyperextensions_to_the_generic() {
    let db = DatabaseConnectionBuilder::default()
        .enable_foreign_keys()
        .build("test")
        .unwrap();
    let before: Vec<_> = schema_ddls()
        .into_iter()
        .filter(|d| d.version < 29)
        .collect();
    db.create_schema(&before).await.unwrap();

    db.run_in_transaction(move |tx| {
        let mut session = common::session(1);
        session.steps = vec![
            common::set(1, 0, (13, 1), 15, 10.0),
            common::set(1, 1, (13, 0), 12, 0.0),
        ];
        common::insert_session(tx, session);
        tx.execute_batch(
            "UPDATE EXERCISE SET notes = 'Slow', reps = '12-15' WHERE category = 13 AND id = 1;
             INSERT INTO WORKOUT (name, enabled) VALUES ('Back', 1), ('Chest', 1);
             INSERT INTO WORKOUT_STEP (workout, idx, kind, exercise_category, exercise_id)
             VALUES ('Back', 0, 'Exercise', 13, 1), ('Chest', 0, 'Exercise', 0, 1);
             INSERT INTO WORKOUT_DEVICE (workout, device, pending)
             VALUES ('Back', 'connect', 0), ('Chest', 'connect', 0);",
        )?;
        Ok(())
    })
    .await
    .unwrap();

    db.create_schema(&schema_ddls()).await.unwrap();

    let mut sets = SessionStepRepository::select().fetch(&db).await.unwrap();
    sets.sort_by_key(|s| s.idx);
    let sets: Vec<_> = sets.iter().map(|s| (s.ex_cat, s.ex_id)).collect();
    assert_eq!(
        sets,
        [(Some(13), Some(GENERIC_EXERCISE_ID)), (Some(13), Some(0))]
    );

    let mut steps = WorkoutStepRepository::select().fetch(&db).await.unwrap();
    steps.sort_by(|a, b| a.workout.cmp(&b.workout));
    let steps: Vec<_> = steps.iter().map(|s| (s.ex_cat, s.ex_id)).collect();
    assert_eq!(
        steps,
        [(Some(13), Some(GENERIC_EXERCISE_ID)), (Some(0), Some(1))]
    );

    let generic = ExerciseRepository::select_by_id(&db, 13, GENERIC_EXERCISE_ID)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        (
            generic.name.as_str(),
            generic.notes.as_deref(),
            generic.reps.as_str()
        ),
        ("", Some("Slow"), "12-15")
    );

    let pending: Vec<(String, bool)> = db
        .run_in_connection(move |conn| {
            let mut stmt =
                conn.prepare("SELECT workout, pending FROM WORKOUT_DEVICE ORDER BY workout")?;
            let rows = stmt
                .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(rows)
        })
        .await
        .unwrap();
    assert_eq!(
        pending,
        [("Back".to_string(), true), ("Chest".to_string(), false)]
    );
}

#[tokio::test]
async fn every_category_has_a_generic_exercise() {
    let db = common::test_db().await;
    let categories = ExerciseCategoryRepository::select()
        .fetch(&db)
        .await
        .unwrap();

    for category in categories {
        assert!(
            ExerciseRepository::select_by_id(&db, category.id, GENERIC_EXERCISE_ID)
                .await
                .unwrap()
                .is_some(),
            "{}",
            category.name
        );
    }
}

#[tokio::test]
async fn catalog_skips_generics_with_a_real_exercise_of_the_same_name() {
    let db = common::test_db().await;
    let catalog = db
        .run_in_connection(move |conn| Ok(exercises_catalog(conn)?))
        .await
        .unwrap();

    // Squats have a real SQUAT exercise; hyperextensions only the generic one
    assert!(!catalog[&28].contains(&GENERIC_EXERCISE_ID));
    assert!(catalog[&13].contains(&GENERIC_EXERCISE_ID));
}

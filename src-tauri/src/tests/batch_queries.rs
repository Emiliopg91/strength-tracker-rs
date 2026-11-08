use rusqlite_orm::{dao::Repository, database::DatabasePool, types::where_clause::Where};

use crate::{
    dao::exercise::{self, ExerciseRepository},
    logic::{
        exercises::fetch_exercises_muscles,
        sessions::{fetch_session_details, fetch_sessions_details},
    },
    tests::common,
};

const BENCH: (u16, u16) = (0, 1);
const CABLE_ROW: (u16, u16) = (23, 20);
const PULL_UP: (u16, u16) = (21, 38);

fn exercises(db: &DatabasePool, keys: &[(u16, u16)]) -> Vec<exercise::Exercise> {
    ExerciseRepository::select()
        .where_(Where::InMultiple(
            vec![
                exercise::entity::columns::CATEGORY,
                exercise::entity::columns::ID,
            ],
            keys.iter()
                .map(|(cat, id)| vec![(*cat).into(), (*id).into()])
                .collect(),
        ))
        .fetch(db)
        .unwrap()
}

/// Muscles of an exercise as sorted `(name, is_primary)` pairs, to compare regardless of order.
fn muscles(exercise: &exercise::Exercise) -> Vec<(String, bool)> {
    let mut muscles: Vec<_> = exercise
        .muscles
        .iter()
        .map(|m| (m.muscle.clone(), m.is_primary))
        .collect();
    muscles.sort();
    muscles
}

#[test]
fn batched_muscles_match_the_relationship_fetcher() {
    let db = common::test_db();
    let mut batched = exercises(&db, &[BENCH, CABLE_ROW, PULL_UP]);
    let mut one_by_one = batched.clone();

    db.run_in_connection(|conn| {
        fetch_exercises_muscles(conn, &mut batched)?;
        for exercise in &mut one_by_one {
            exercise.fetch_muscles_relationship_in(conn)?;
        }
        Ok(())
    })
    .unwrap();

    assert!(batched.iter().any(|e| !e.muscles.is_empty()));
    for (batched, single) in batched.iter().zip(&one_by_one) {
        assert_eq!(muscles(batched), muscles(single));
    }
}

/// Details as JSON with their muscle lists sorted, as they come from a `HashSet` in any order.
fn normalized(details: &[crate::dto::sessions::SessionDetails]) -> serde_json::Value {
    let mut value = serde_json::to_value(details).unwrap();
    for detail in value.as_array_mut().unwrap() {
        for key in ["primary_muscles", "secondary_muscles"] {
            detail[key]
                .as_array_mut()
                .unwrap()
                .sort_by_key(|m| m.as_str().unwrap().to_string());
        }
    }
    value
}

#[test]
fn batched_session_details_match_single_ones_in_the_given_order() {
    let db = common::test_db();
    db.run_in_transaction(|tx| {
        for (date, exercise) in [(1, BENCH), (2, CABLE_ROW)] {
            let mut session = common::session(date);
            session.steps = vec![
                common::set(date, 0, exercise, 10, 50.0),
                common::set(date, 1, exercise, 8, 55.0),
            ];
            session.laps = vec![common::lap(date, 0, None)];
            session.additional_data = Some(common::additional_data(date));
            common::insert_session(tx, session);
        }
        Ok(())
    })
    .unwrap();

    let (batched, single) = db
        .run_in_connection(|conn| {
            let batched = fetch_sessions_details(conn, &[2, 1], true, true)?;
            let single = vec![
                fetch_session_details(conn, 2, true, true)?,
                fetch_session_details(conn, 1, true, true)?,
            ];
            Ok((batched, single))
        })
        .unwrap();

    let batched = normalized(&batched);
    assert_eq!(batched, normalized(&single));
    assert_eq!(batched[0]["timestamp"], 2);
    // Each session only gets the muscles of its own exercises
    assert_ne!(batched[0]["primary_muscles"], batched[1]["primary_muscles"]);
}

#[test]
fn no_sessions_give_no_details() {
    let db = common::test_db();

    let details = db
        .run_in_connection(|conn| Ok(fetch_sessions_details(conn, &[], true, true)?))
        .unwrap();

    assert!(details.is_empty());
}

use rusqlite_orm::dao::Repository;
use serde_json::Value;

use crate::{
    dao::{
        additional_data::AdditionalData,
        body_metric::{BodyMetric, BodyMetricRepository},
    },
    dto::export::Export,
    tests::common,
    utils::translations::Languages,
};

fn export_json(db: &rusqlite_orm::database::DatabasePool) -> Value {
    let mut json = Vec::new();
    Export::from_database(db, Languages::English)
        .unwrap()
        .write_json(&mut json)
        .unwrap();
    serde_json::from_slice(&json).unwrap()
}

#[test]
fn empty_database_export() {
    let db = common::test_db();
    let json = export_json(&db);

    for key in ["body_metrics", "exercises", "workouts", "sessions", "laps"] {
        assert_eq!(json[key].as_array().unwrap().len(), 0, "{key}");
    }
    assert!(!json["settings"].as_array().unwrap().is_empty());

    // Migration 021 seeds the Garmin Connect pseudo-device
    let devices = json["devices"].as_array().unwrap();
    assert_eq!(devices.len(), 1);
    assert_eq!(devices[0]["serial"], "connect");
}

#[test]
fn export_contains_all_data() {
    let db = common::test_db();

    let mut strength = common::session(1);
    strength.sets = vec![
        common::set(1, 0, (28, 0), 5, 100.0),
        common::set(1, 1, (28, 0), 5, 100.0),
        common::set(1, 2, (0, 0), 8, 60.0),
    ];

    let mut run = common::session(2);
    run.laps = vec![common::lap(2, 0, Some((0, 0)))];
    let mut data = common::additional_data(2);
    data.coordinates = Some(AdditionalData::build_coordinates_blob(&[(0, 0)]));
    data.heart_rates = Some(vec![120]);
    run.additional_data = Some(data);

    db.run_in_transaction(|tx| {
        common::insert_session(tx, strength.clone());
        common::insert_session(tx, run.clone());
        BodyMetricRepository::insert()
            .item(&mut BodyMetric {
                date: 1,
                weight: 80.0,
                fat_ratio: 18.0,
                lean_mass: 62.0,
                water_ratio: 55.0,
            })
            .execute_in(tx)?;
        Ok(())
    })
    .unwrap();

    let json = export_json(&db);

    assert_eq!(json["body_metrics"].as_array().unwrap().len(), 1);
    assert_eq!(json["laps"].as_array().unwrap().len(), 1);
    assert_eq!(
        json["exercises"].as_array().unwrap().len(),
        2,
        "only used exercises are exported"
    );

    let sessions = json["sessions"].as_array().unwrap();
    assert_eq!(sessions.len(), 2);

    let strength = sessions.iter().find(|s| s["date"] == 1).unwrap();
    assert_eq!(strength["sets"].as_array().unwrap().len(), 3);
    assert!(strength.get("coordinates").is_none());

    let run = sessions.iter().find(|s| s["date"] == 2).unwrap();
    assert!(run.get("sets").is_none());
    assert_eq!(run["coordinates"].as_array().unwrap().len(), 1);
    assert_eq!(run["heart_rates"][0], 120);
}

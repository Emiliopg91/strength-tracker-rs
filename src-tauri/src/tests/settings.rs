use rusqlite_orm::dao::Repository;
use semver::Version;

use crate::{
    dao::settings::{DistanceUnit, Settings, SettingsRepository, WeightUnit, settings_keys},
    dto::app,
    tests::common,
    utils::translations::Languages,
};

#[test]
fn units_parse_and_display_roundtrip() {
    for unit in [DistanceUnit::Kilometers, DistanceUnit::Miles] {
        let parsed = DistanceUnit::try_from(unit.to_string().as_str()).unwrap();
        assert_eq!(parsed.to_string(), unit.to_string());
    }
    for unit in [WeightUnit::Kilograms, WeightUnit::Pounds] {
        let parsed = WeightUnit::try_from(unit.to_string().as_str()).unwrap();
        assert_eq!(parsed.to_string(), unit.to_string());
    }

    assert!(DistanceUnit::try_from("Parsecs").is_err());
    assert!(WeightUnit::try_from("kilograms").is_err());
}

#[tokio::test]
async fn seeded_defaults() {
    let db = common::test_db().await;

    assert!(!Settings::get_start_on_boot(&db).await);
    assert!(!Settings::get_on_device_connect(&db).await);
    assert!(matches!(
        Settings::get_weight_unit(&db).await,
        WeightUnit::Kilograms
    ));
    assert!(matches!(
        Settings::get_distance_unit(&db).await,
        DistanceUnit::Kilometers
    ));
    assert_eq!(Settings::get_version(&db).await, Version::new(0, 1, 0));
}

#[tokio::test]
async fn setters_roundtrip() {
    let db = common::test_db().await;

    for (name, value) in [
        (settings_keys::START_ON_BOOT, "true".to_string()),
        (settings_keys::ON_DEVICE_CONNECT, "true".to_string()),
        (settings_keys::WEIGHT_UNIT, WeightUnit::Pounds.to_string()),
        (
            settings_keys::DISTANCE_UNIT,
            DistanceUnit::Miles.to_string(),
        ),
        (settings_keys::LANGUAGE, Languages::Spanish.to_string()),
        (settings_keys::VERSION, Version::new(2, 5, 0).to_string()),
    ] {
        Settings::set(&db, name, value).await.unwrap();
    }

    assert!(Settings::get_start_on_boot(&db).await);
    assert!(Settings::get_on_device_connect(&db).await);
    assert!(matches!(
        Settings::get_weight_unit(&db).await,
        WeightUnit::Pounds
    ));
    assert!(matches!(
        Settings::get_distance_unit(&db).await,
        DistanceUnit::Miles
    ));
    assert!(matches!(
        Settings::get_language(&db).await,
        Languages::Spanish
    ));
    assert_eq!(Settings::get_version(&db).await, Version::new(2, 5, 0));

    let settings = app::Settings::load(&db).await;
    assert!(settings.start_boot);
    assert!(settings.on_device_connect);
    assert!(matches!(settings.weight_unit, WeightUnit::Pounds));
    assert!(matches!(settings.distance_unit, DistanceUnit::Miles));
    assert!(matches!(settings.language, Languages::Spanish));
}

#[tokio::test]
async fn invalid_stored_values_fall_back_to_defaults() {
    let db = common::test_db().await;

    for (name, value) in [
        (settings_keys::WEIGHT_UNIT, "Stones"),
        (settings_keys::DISTANCE_UNIT, "Leagues"),
        (settings_keys::VERSION, "not-a-version"),
    ] {
        SettingsRepository::insert()
            .or_replace()
            .item(&mut Settings {
                name: name.to_string(),
                value: value.to_string(),
            })
            .execute(&db)
            .await
            .unwrap();
    }

    assert!(matches!(
        Settings::get_weight_unit(&db).await,
        WeightUnit::Kilograms
    ));
    assert!(matches!(
        Settings::get_distance_unit(&db).await,
        DistanceUnit::Kilometers
    ));
    assert_eq!(Settings::get_version(&db).await, Version::new(0, 1, 0));
}

#[tokio::test]
async fn save_persists_every_setting() {
    let db = common::test_db().await;

    let mut settings = app::Settings::load(&db).await;
    settings.start_boot = true;
    settings.close_to_tray = true;
    settings.start_into_tray = true;
    settings.on_device_connect = true;
    settings.weight_unit = WeightUnit::Pounds;
    settings.distance_unit = DistanceUnit::Miles;
    settings.language = Languages::Spanish;
    settings.save(&db).await.unwrap();

    let loaded = app::Settings::load(&db).await;
    assert!(loaded.start_boot);
    assert!(loaded.close_to_tray);
    assert!(loaded.start_into_tray);
    assert!(loaded.on_device_connect);
    assert!(matches!(loaded.weight_unit, WeightUnit::Pounds));
    assert!(matches!(loaded.distance_unit, DistanceUnit::Miles));
    assert!(matches!(loaded.language, Languages::Spanish));
}

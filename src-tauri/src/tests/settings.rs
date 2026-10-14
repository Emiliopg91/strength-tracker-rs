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

#[test]
fn seeded_defaults() {
    let db = common::test_db();

    assert!(!Settings::get_start_on_boot(&db));
    assert!(!Settings::get_on_device_connect(&db));
    assert!(matches!(
        Settings::get_weight_unit(&db),
        WeightUnit::Kilograms
    ));
    assert!(matches!(
        Settings::get_distance_unit(&db),
        DistanceUnit::Kilometers
    ));
    assert_eq!(Settings::get_version(&db), Version::new(0, 1, 0));
}

#[test]
fn setters_roundtrip() {
    let db = common::test_db();

    Settings::set_start_on_boot(&db, true).unwrap();
    Settings::set_on_device_connect(&db, true).unwrap();
    Settings::set_weight_unit(&db, &WeightUnit::Pounds).unwrap();
    Settings::set_distance_unit(&db, &DistanceUnit::Miles).unwrap();
    Settings::set_language(&db, &Languages::Spanish).unwrap();
    Settings::set_version(&db, &Version::new(2, 5, 0)).unwrap();

    assert!(Settings::get_start_on_boot(&db));
    assert!(Settings::get_on_device_connect(&db));
    assert!(matches!(Settings::get_weight_unit(&db), WeightUnit::Pounds));
    assert!(matches!(
        Settings::get_distance_unit(&db),
        DistanceUnit::Miles
    ));
    assert!(matches!(Settings::get_language(&db), Languages::Spanish));
    assert_eq!(Settings::get_version(&db), Version::new(2, 5, 0));

    let settings = app::Settings::from(&db);
    assert!(settings.start_boot);
    assert!(settings.on_device_connect);
    assert!(matches!(settings.weight_unit, WeightUnit::Pounds));
    assert!(matches!(settings.distance_unit, DistanceUnit::Miles));
    assert!(matches!(settings.language, Languages::Spanish));
}

#[test]
fn invalid_stored_values_fall_back_to_defaults() {
    let db = common::test_db();

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
            .unwrap();
    }

    assert!(matches!(
        Settings::get_weight_unit(&db),
        WeightUnit::Kilograms
    ));
    assert!(matches!(
        Settings::get_distance_unit(&db),
        DistanceUnit::Kilometers
    ));
    assert_eq!(Settings::get_version(&db), Version::new(0, 1, 0));
}

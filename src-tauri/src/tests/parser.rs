use std::fs;

use crate::{
    dao::{additional_data::AdditionalData, session::Session},
    fit::parser::{FitParser, errors::ParseFitFileError},
    tests::common::{STRENGTH_SPORT, STRENGTH_SUB_SPORT},
};

#[test]
fn missing_file_fails_to_open() {
    let res = FitParser::from_file("/nonexistent/path/activity.fit");
    assert!(matches!(res, Err(ParseFitFileError::FileOpening(..))));
}

#[test]
fn garbage_file_fails_to_parse() {
    let path = std::env::temp_dir().join(format!("gtrs-test-garbage-{}.fit", std::process::id()));
    fs::write(
        &path,
        b"this is definitely not a FIT file, just some random bytes",
    )
    .unwrap();

    let res = FitParser::from_file(&path).and_then(Session::try_from);
    let _ = fs::remove_file(&path);

    assert!(matches!(res, Err(ParseFitFileError::FileReading(..))));
}

// Fixtures are real Garmin recordings anonymized with `scripts/anonymize_fit.py`: GPS tracks were
// moved to Oslo, Buenos Aires and Kyoto respectively. `indoor_biking` was re-encoded with rustyfit
// from a JSON dump of the original recording, as the original file was no longer available.

/// Walking sport/sub-sport pair, as recorded by the device.
const WALKING_SPORT: u8 = 11;
const WALKING_SUB_SPORT: u8 = 0;

/// Indoor cycling sport/sub-sport pair, as recorded by the device.
const CYCLING_SPORT: u8 = 2;
const INDOOR_CYCLING_SUB_SPORT: u8 = 6;

fn parse_fixture(name: &str) -> Session {
    let path = format!(
        "{}/src/tests/fixtures/{name}.fit",
        env!("CARGO_MANIFEST_DIR")
    );
    FitParser::from_file(&path)
        .and_then(Session::try_from)
        .unwrap_or_else(|e| panic!("{name}: {e:?}"))
}

struct Expected {
    fixture: &'static str,
    date: u32,
    name: &'static str,
    total_elapsed_time: u32,
    active_time: u32,
    active_calories: u16,
    training_load: u16,
    sets: usize,
    records: usize,
}

const STRENGTH: [Expected; 6] = [
    Expected {
        fixture: "strength_1",
        date: 1781167080,
        name: "3 - Pierna",
        total_elapsed_time: 3230,
        active_time: 2012,
        active_calories: 272,
        training_load: 62,
        sets: 33,
        records: 1377,
    },
    Expected {
        fixture: "strength_2",
        date: 1781254355,
        name: "4 - Torso",
        total_elapsed_time: 4207,
        active_time: 1769,
        active_calories: 252,
        training_load: 73,
        sets: 39,
        records: 1339,
    },
    Expected {
        fixture: "strength_3",
        date: 1781511466,
        name: "1 - Pierna",
        total_elapsed_time: 3179,
        active_time: 1958,
        active_calories: 248,
        training_load: 65,
        sets: 33,
        records: 1631,
    },
    Expected {
        fixture: "strength_4",
        date: 1781682701,
        name: "2 - Torso",
        total_elapsed_time: 3269,
        active_time: 1614,
        active_calories: 246,
        training_load: 59,
        sets: 33,
        records: 1313,
    },
    Expected {
        fixture: "strength_5",
        date: 1781769160,
        name: "3 - Pierna",
        total_elapsed_time: 3806,
        active_time: 2538,
        active_calories: 293,
        training_load: 76,
        sets: 33,
        records: 1848,
    },
    Expected {
        fixture: "strength_6",
        date: 1788160480,
        name: "1 - Pierna",
        total_elapsed_time: 2604,
        active_time: 1386,
        active_calories: 245,
        training_load: 82,
        sets: 33,
        records: 1373,
    },
];

const WALKS: [(Expected, (f64, f64), f64, usize); 3] = [
    (
        Expected {
            fixture: "walk_oslo",
            date: 1786981687,
            name: "",
            total_elapsed_time: 4005,
            active_time: 0,
            active_calories: 159,
            training_load: 6,
            sets: 0,
            records: 693,
        },
        (59.9139, 10.7522),
        2638.05,
        0,
    ),
    (
        Expected {
            fixture: "walk_buenos_aires",
            date: 1788893128,
            name: "",
            total_elapsed_time: 836,
            active_time: 0,
            active_calories: 70,
            training_load: 18,
            sets: 0,
            records: 184,
        },
        (-34.6037, -58.3816),
        1189.13,
        1,
    ),
    (
        Expected {
            fixture: "walk_kyoto",
            date: 1789621646,
            name: "",
            total_elapsed_time: 7242,
            active_time: 0,
            active_calories: 169,
            training_load: 5,
            sets: 0,
            records: 1249,
        },
        (35.0116, 135.7681),
        3147.47,
        3,
    ),
];

fn assert_session(session: &Session, expected: &Expected, sport: (u8, u8)) {
    let ctx = expected.fixture;
    assert_eq!(session.date, expected.date, "{ctx}");
    assert_eq!(session.name, expected.name, "{ctx}");
    assert_eq!(
        session.total_elapsed_time, expected.total_elapsed_time,
        "{ctx}"
    );
    assert_eq!(session.active_time, expected.active_time, "{ctx}");
    assert_eq!(session.active_calories, expected.active_calories, "{ctx}");
    assert_eq!(session.training_load, expected.training_load, "{ctx}");
    assert_eq!((session.sport, session.sub_sport), sport, "{ctx}");
    assert_eq!(session.sets.len(), expected.sets, "{ctx}");

    let additional_data = session.additional_data.as_ref().expect(ctx);
    assert_eq!(additional_data.session, expected.date, "{ctx}");
    assert_eq!(
        additional_data.heart_rates.as_ref().map(Vec::len),
        Some(expected.records),
        "{ctx}"
    );
}

#[test]
fn parses_strength_sessions() {
    for expected in &STRENGTH {
        let session = parse_fixture(expected.fixture);
        assert_session(&session, expected, (STRENGTH_SPORT, STRENGTH_SUB_SPORT));

        assert_eq!(session.workout.as_deref(), Some(expected.name));
        assert!(session.laps.is_empty(), "{}", expected.fixture);

        let workout = session.workout_obj.as_ref().unwrap();
        assert_eq!(workout.name, expected.name, "{}", expected.fixture);
        assert!(!workout.steps.is_empty(), "{}", expected.fixture);
        for (idx, step) in workout.steps.iter().enumerate() {
            assert_eq!(step.workout, expected.name, "{}", expected.fixture);
            assert_eq!(step.idx as usize, idx, "{}", expected.fixture);
            if let Some(begin) = step.begin_idx {
                assert!(begin < step.idx, "{}", expected.fixture);
            }
        }

        for (idx, set) in session.sets.iter().enumerate() {
            assert_eq!(set.session, expected.date);
            assert_eq!(set.idx as usize, idx);
            assert_eq!(set.exercise.is_some(), set.ex_cat != u16::MAX);
        }

        let additional_data = session.additional_data.unwrap();
        assert!(
            additional_data.coordinates.is_none(),
            "{}",
            expected.fixture
        );
        assert!(additional_data.speeds.is_none(), "{}", expected.fixture);
        assert!(additional_data.altitudes.is_none(), "{}", expected.fixture);
        assert_eq!(additional_data.distance, Some(0.0));
    }
}

#[test]
fn parses_strength_sets_in_order() {
    let session = parse_fixture("strength_1");
    let sets: Vec<_> = session
        .sets
        .iter()
        .take(4)
        .map(|s| (s.ex_cat, s.ex_id, s.reps, s.weight))
        .collect();

    assert_eq!(
        sets,
        [
            (8, 0, 10, 70.0),
            (17, 27, 20, 36.0),
            (u16::MAX, 0, 0, 0.0),
            (8, 0, 8, 70.0)
        ]
    );
}

#[test]
fn parses_strength_sets_with_fractional_weight() {
    let session = parse_fixture("strength_2");
    let last = session.sets.iter().rfind(|s| s.ex_cat != u16::MAX).unwrap();

    assert_eq!(
        (last.ex_cat, last.ex_id, last.reps, last.weight),
        (37, 32, 15, 13.5)
    );
}

#[test]
fn parses_walk_sessions() {
    for (expected, city, distance, laps) in &WALKS {
        let ctx = expected.fixture;
        let session = parse_fixture(ctx);
        assert_session(&session, expected, (WALKING_SPORT, WALKING_SUB_SPORT));

        assert!(session.workout.is_none(), "{ctx}");
        assert!(session.workout_obj.is_none(), "{ctx}");

        // The first lap is dropped by the parser
        assert_eq!(session.laps.len(), *laps, "{ctx}");
        for (idx, lap) in session.laps.iter().enumerate() {
            assert_eq!(lap.session, expected.date);
            assert_eq!(lap.idx as usize, idx);
            let lat = lap.start_latitude.unwrap() as f64 * AdditionalData::SEMICIRCLE_TO_DEGREES;
            let lon = lap.start_longitude.unwrap() as f64 * AdditionalData::SEMICIRCLE_TO_DEGREES;
            assert!(
                (lat - city.0).abs() < 0.1 && (lon - city.1).abs() < 0.1,
                "{ctx}: lap at {lat},{lon}"
            );
        }

        let additional_data = session.additional_data.unwrap();
        assert_eq!(additional_data.distance, Some(*distance), "{ctx}");
        assert_eq!(
            additional_data.get_speeds().map(|v| v.len()),
            Some(expected.records),
            "{ctx}"
        );
        assert_eq!(
            additional_data.get_altitudes().map(|v| v.len()),
            Some(expected.records),
            "{ctx}"
        );

        let coords = additional_data.get_coordinates_degrees().unwrap();
        assert_eq!(coords.len(), expected.records, "{ctx}");

        // The track starts exactly on the city it was moved to and stays around it
        let (first_lat, first_lon) = coords.iter().flatten().next().copied().unwrap();
        assert!(
            (first_lat - city.0).abs() < 1e-6 && (first_lon - city.1).abs() < 1e-6,
            "{ctx}"
        );
        for (lat, lon) in coords.iter().flatten() {
            assert!(
                (lat - city.0).abs() < 0.1 && (lon - city.1).abs() < 0.1,
                "{ctx}: point at {lat},{lon}"
            );
        }
    }
}

#[test]
fn parses_indoor_biking_session() {
    let expected = Expected {
        fixture: "indoor_biking",
        date: 1703262819,
        name: "",
        total_elapsed_time: 4180,
        active_time: 0,
        active_calories: 313,
        training_load: 51,
        sets: 0,
        records: 3773,
    };
    let ctx = expected.fixture;
    let session = parse_fixture(ctx);
    assert_session(
        &session,
        &expected,
        (CYCLING_SPORT, INDOOR_CYCLING_SUB_SPORT),
    );

    assert!(session.workout.is_none(), "{ctx}");
    assert!(session.workout_obj.is_none(), "{ctx}");
    assert!(session.laps.is_empty(), "{ctx}");

    // Distance and speed come from the speed sensor, but there is no GPS nor barometer data
    let additional_data = session.additional_data.unwrap();
    assert_eq!(additional_data.distance, Some(19731.78), "{ctx}");
    let speeds = additional_data.get_speeds().unwrap();
    assert_eq!(speeds.len(), expected.records, "{ctx}");
    assert_eq!(speeds.iter().flatten().count(), 3739, "{ctx}");
    assert!(additional_data.coordinates.is_none(), "{ctx}");
    assert!(additional_data.altitudes.is_none(), "{ctx}");
}

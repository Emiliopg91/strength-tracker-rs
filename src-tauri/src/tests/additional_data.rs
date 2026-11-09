use crate::{dao::session_data::SessionData, tests::common};

const QUARTER_TURN: i32 = 1 << 30;

#[test]
fn coordinates_blob_roundtrip() {
    let points = [(0, 0), (123_456, -654_321), (i32::MIN + 1, i32::MAX - 1)];

    let mut data = common::additional_data(1);
    data.coordinates = Some(SessionData::build_coordinates_blob(&points));

    let decoded = data.get_coordinates_semicircle().unwrap();
    assert_eq!(
        decoded,
        points.iter().copied().map(Some).collect::<Vec<_>>()
    );
}

#[test]
fn coordinates_invalid_position_is_none() {
    let invalid = SessionData::INVALID_POSITION;
    let points = [
        (1, 2),
        (invalid, 5),
        (5, invalid),
        (invalid, invalid),
        (3, 4),
    ];

    let mut data = common::additional_data(1);
    data.coordinates = Some(SessionData::build_coordinates_blob(&points));

    assert_eq!(
        data.get_coordinates_semicircle().unwrap(),
        vec![Some((1, 2)), None, None, None, Some((3, 4))]
    );
}

#[test]
fn coordinates_to_degrees() {
    let points = [(0, 0), (QUARTER_TURN, -QUARTER_TURN)];

    let mut data = common::additional_data(1);
    data.coordinates = Some(SessionData::build_coordinates_blob(&points));

    let degrees = data.get_coordinates_degrees().unwrap();
    assert_eq!(degrees[0], Some((0.0, 0.0)));
    let (lat, lon) = degrees[1].unwrap();
    assert!((lat - 90.0).abs() < 1e-9);
    assert!((lon + 90.0).abs() < 1e-9);
}

#[test]
fn coordinates_trailing_bytes_are_ignored() {
    let mut blob = SessionData::build_coordinates_blob(&[(7, 8)]);
    blob.extend_from_slice(&[1, 2, 3]);

    let mut data = common::additional_data(1);
    data.coordinates = Some(blob);

    assert_eq!(
        data.get_coordinates_semicircle().unwrap(),
        vec![Some((7, 8))]
    );
}

#[test]
fn heart_rates_invalid_is_none() {
    let mut data = common::additional_data(1);
    data.heart_rates = Some(vec![60, 0, SessionData::INVALID_HEAR_RATE, 254]);

    assert_eq!(
        data.get_heart_rates().unwrap(),
        vec![Some(60), Some(0), None, Some(254)]
    );
}

#[test]
fn missing_blobs_are_none() {
    let data = common::additional_data(1);

    assert!(data.get_coordinates_semicircle().is_none());
    assert!(data.get_coordinates_degrees().is_none());
    assert!(data.get_heart_rates().is_none());
}

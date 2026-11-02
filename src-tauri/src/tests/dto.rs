use crate::{
    dao::{additional_data::AdditionalData, body_metric::BodyMetric, device::Device},
    dto::{
        body_metrics::BodyMetricListItem,
        sessions::{SessionDetails, SessionListItem, SessionSet},
    },
    tests::common,
    utils::time::local_midnight,
};

#[test]
fn session_list_item_active_calories() {
    let item = SessionListItem::from(&common::session(1_700_000_000));

    assert_eq!(item.timestamp, 1_700_000_000);
    assert_eq!(item.active_calories, 100);
    assert_eq!(item.training_load, 80);
    assert!(!item.has_record);
}

#[test]
fn session_set_from_set() {
    let mut set = common::set(1, 3, (5, 7), 8, 62.5);
    set.pr = true;

    let dto = SessionSet::from(&set);
    assert_eq!((dto.ex_cat, dto.ex_id, dto.idx), (5, 7, 3));
    assert_eq!((dto.reps, dto.weight, dto.pr), (8, 62.5, true));
}

#[test]
fn body_metric_roundtrip() {
    let metric = BodyMetric {
        date: 1_700_000_000,
        weight: 80.5,
        fat_ratio: 18.2,
        lean_mass: 62.1,
        water_ratio: 55.0,
    };

    let item = BodyMetricListItem::from(&metric);
    let back = BodyMetric::try_from(&item).unwrap();

    assert_eq!(back.date, local_midnight(metric.date as i64).unwrap());
    assert_eq!(back.weight, metric.weight);
    assert_eq!(back.fat_ratio, metric.fat_ratio);
    assert_eq!(back.lean_mass, metric.lean_mass);
    assert_eq!(back.water_ratio, metric.water_ratio);
}

#[test]
fn session_details_without_additional_data() {
    let session = common::session(1);
    let sets = [common::set(1, 0, (0, 0), 10, 50.0)];

    let details = SessionDetails::from((&session, &[][..], &sets[..], &[][..]));

    assert_eq!(details.timestamp, 1);
    assert_eq!(details.sets.len(), 1);
    assert!(details.heart_rates.is_empty());
    assert!(details.coordinates.is_empty());
    assert!(details.distance.is_none());
    assert!(details.notes.is_empty());
    assert!(details.device.is_none());
}

#[test]
fn session_details_with_additional_data() {
    let mut session = common::session(1);
    session.device_obj = Some(Device {
        serial: "123".to_string(),
        model: "Forerunner 265".to_string(),
        last_activity_sync: None,
        last_body_metric_sync: None,
        usb: true,
        connect: false,
        strength_workout: true,
    });

    let mut data = common::additional_data(1);
    data.heart_rates = Some(vec![100, AdditionalData::INVALID_HEAR_RATE]);
    data.coordinates = Some(AdditionalData::build_coordinates_blob(&[(1, 2)]));
    data.distance = Some(5_250.0);
    data.notes = Some("Easy run".to_string());
    session.additional_data = Some(data);

    let details = SessionDetails::from((&session, &[][..], &[][..], &[][..]));

    assert_eq!(details.heart_rates, vec![Some(100), None]);
    assert_eq!(details.coordinates, vec![Some((1, 2))]);
    assert_eq!(details.distance, Some(5.25));
    assert_eq!(details.notes, "Easy run");
    assert_eq!(details.device.as_deref(), Some("Garmin Forerunner 265"));
}

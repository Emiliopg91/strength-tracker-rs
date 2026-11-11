use chrono::{Datelike, Days, Local, TimeZone};

use crate::{
    logic::sessions::queries::{heatmap_data, session_set_flags},
    tests::common,
};

/// Builds a timestamp for `days_ago` days before today, anchored at local noon so the
/// resulting instant never lands on a different calendar day due to DST shifts.
fn timestamp_days_ago(days_ago: u64) -> u32 {
    let date = Local::now()
        .date_naive()
        .checked_sub_days(Days::new(days_ago))
        .unwrap();
    let noon = date.and_hms_opt(12, 0, 0).unwrap();
    Local
        .from_local_datetime(&noon)
        .earliest()
        .unwrap()
        .timestamp() as u32
}

#[tokio::test]
async fn heatmap_totals_and_record_flags_match_inserted_sessions() {
    let db = common::test_db().await;

    // 20 sessions spread over the last year, far enough apart that each lands on its
    // own calendar day (no two sessions share a month/day cell).
    let days_ago: Vec<u64> = (0..20).map(|i| 5 + i * 17).collect();
    assert!(days_ago.iter().all(|&d| d < 365));

    // A subset of those sessions also set a personal record.
    let record_indices: [usize; 6] = [1, 4, 7, 10, 13, 17];

    let tx_days_ago = days_ago.clone();
    db.run_in_transaction(move |tx| {
        for (i, &days) in tx_days_ago.iter().enumerate() {
            let date = timestamp_days_ago(days);
            let mut session = common::session(date);
            session.training_load = 40 + (i as u16) * 7;

            if record_indices.contains(&i) {
                let mut set = common::set(date, 0, (0, i as u16), 5, 100.0 + i as f32);
                set.pr = true;
                session.steps = vec![set];
            }

            common::insert_session(tx, session);
        }
        Ok(())
    })
    .await
    .unwrap();

    let heatmap = heatmap_data(&db).await.unwrap();

    for (i, &days) in days_ago.iter().enumerate() {
        let date = Local::now()
            .date_naive()
            .checked_sub_days(Days::new(days))
            .unwrap();
        let cell = &heatmap[date.month0() as usize][date.day0() as usize];

        let expected_load = (40 + (i as u16) * 7) as u32;
        let expected_record = record_indices.contains(&i);

        assert_eq!(
            cell.0, expected_load,
            "session {i} ({date}) training load mismatch"
        );
        assert_eq!(
            cell.1,
            if expected_record { 1 } else { 0 },
            "session {i} ({date}) record count mismatch"
        );
    }
}

#[tokio::test]
async fn session_set_flags_report_sets_and_records_per_session() {
    let db = common::test_db().await;

    db.run_in_transaction(move |tx| {
        // No sets at all.
        common::insert_session(tx, common::session(1_000));

        // Sets, none of them a record.
        let mut plain = common::session(2_000);
        plain.steps = vec![common::set(2_000, 0, (0, 1), 5, 50.0)];
        common::insert_session(tx, plain);

        // Sets, one of them a record.
        let mut record = common::session(3_000);
        let mut pr = common::set(3_000, 1, (0, 2), 5, 60.0);
        pr.pr = true;
        record.steps = vec![common::set(3_000, 0, (0, 1), 5, 40.0), pr];
        common::insert_session(tx, record);

        Ok(())
    })
    .await
    .unwrap();

    let all = db
        .run_in_connection(move |conn| Ok(session_set_flags(conn, None, None)?))
        .await
        .unwrap();
    assert_eq!(all.len(), 2);
    assert_eq!(all.get(&2_000), Some(&false));
    assert_eq!(all.get(&3_000), Some(&true));
    assert!(!all.contains_key(&1_000));

    let recent = db
        .run_in_connection(move |conn| Ok(session_set_flags(conn, Some(2_500), None)?))
        .await
        .unwrap();
    assert_eq!(recent.len(), 1);
    assert_eq!(recent.get(&3_000), Some(&true));

    let bounded = db
        .run_in_connection(move |conn| Ok(session_set_flags(conn, Some(2_500), Some(3_500))?))
        .await
        .unwrap();
    assert_eq!(bounded.len(), 1);
    assert_eq!(bounded.get(&3_000), Some(&true));

    let upto = db
        .run_in_connection(move |conn| Ok(session_set_flags(conn, None, Some(2_500))?))
        .await
        .unwrap();
    assert_eq!(upto.len(), 1);
    assert_eq!(upto.get(&2_000), Some(&false));
}

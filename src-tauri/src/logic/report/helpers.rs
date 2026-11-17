use std::collections::BTreeSet;

use crate::{
    dao::{body_metric::BodyMetric, session::Session},
    dto::{body_metrics::BodyMetricListItem, report::ReportData},
    logic::{exercises::helpers::exercise_details, sessions::helpers::fetch_sessions_details},
};

const STRENGTH_SPORT: u8 = 10;

const STRENGTH_SUB_SPORT: u8 = 20;

/// Strength sessions between `from` and `to`, with the history of every exercise trained in them
/// and the body measurements of the period.
pub(super) async fn report_data(
    conn: &rusqlite_orm::rusqlite::Connection,
    from: u32,
    to: u32,
) -> rusqlite_orm::errors::Result<ReportData> {
    // Oldest first
    let timestamps: Vec<u32> = Session::get_sessions(
        conn,
        Some(from.into()),
        Some(to.into()),
        Some(STRENGTH_SPORT),
        Some(STRENGTH_SUB_SPORT),
    )
    .await?
    .iter()
    .rev()
    .map(|s| s.date)
    .collect();
    let session_details = fetch_sessions_details(conn, &timestamps, false, false).await?;

    // Sorted and deduplicated; sets without exercise are skipped
    let trained: BTreeSet<(u16, u16)> = session_details
        .iter()
        .flat_map(|d| &d.steps)
        .filter_map(|set| Some((set.ex_cat?, set.ex_id?)))
        .collect();

    // Loaded once, by the first exercise needing them, and shared by the rest
    let mut all_body_metrics = None;
    let mut exercises = Vec::new();
    for (category, id) in trained {
        if let Ok(mut details) =
            exercise_details(conn, category, id, Some((from, to)), &mut all_body_metrics).await
        {
            details.workouts.reverse();
            exercises.push(details);
        }
    }

    let mut body_metrics = BodyMetric::get_by_range(conn, Some((from, to)))
        .await?
        .iter()
        .map(BodyMetricListItem::from)
        .collect::<Vec<_>>();
    body_metrics.reverse();

    Ok(ReportData {
        sessions: session_details,
        exercises,
        body_metrics,
    })
}

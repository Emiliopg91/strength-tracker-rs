use std::collections::BTreeSet;

use rusqlite_orm::{
    dao::Repository,
    types::{order_by::OrderBy, where_clause::Where},
};

use super::helpers::{STRENGTH_SPORT, STRENGTH_SUB_SPORT};
use crate::{
    dao::session::{self, SessionRepository},
    dto::report::ReportData,
    logic::{
        body_metrics::queries::fetch_body_measures, exercises::queries::exercise_details,
        sessions::queries::fetch_sessions_details,
    },
    utils::translations::{Languages, translate},
};

/// Strength sessions between `from` and `to`, with the history of every exercise trained in them
/// and the body measurements of the period.
pub fn report_data(
    conn: &rusqlite_orm::rusqlite::Connection,
    from: u32,
    to: u32,
    lang: Languages,
) -> rusqlite_orm::errors::Result<ReportData> {
    let timestamps: Vec<u32> = SessionRepository::select()
        .columns(&[session::entity::columns::DATE])
        .where_(Where::And(vec![
            Where::Gte(session::entity::columns::DATE, from.into()),
            Where::Lte(session::entity::columns::DATE, to.into()),
            Where::Eq(session::entity::columns::SPORT, STRENGTH_SPORT.into()),
            Where::Eq(
                session::entity::columns::SUB_SPORT,
                STRENGTH_SUB_SPORT.into(),
            ),
        ]))
        .order_by(OrderBy::Asc(session::entity::columns::DATE))
        .fetch_in(conn)?
        .iter()
        .map(|row| row.get_u32(session::entity::columns::DATE.as_ref()))
        .collect();
    let session_details = fetch_sessions_details(conn, &timestamps, false, false)?;

    // Sorted and deduplicated; sets without exercise are skipped
    let trained: BTreeSet<(u16, u16)> = session_details
        .iter()
        .flat_map(|d| &d.steps)
        .filter_map(|set| Some((set.ex_cat?, set.ex_id?)))
        .collect();

    // Loaded once, by the first exercise needing them, and shared by the rest
    let mut all_body_metrics = None;
    let mut exercises: Vec<_> = trained
        .into_iter()
        .filter_map(|(category, id)| {
            if let Ok(mut details) =
                exercise_details(conn, category, id, Some((from, to)), &mut all_body_metrics)
            {
                details.workouts.reverse();
                Some(details)
            } else {
                None
            }
        })
        .collect();
    exercises.sort_by_key(|e| translate(&format!("exercise_{}_{}", e.category, e.id), lang));

    let mut body_metrics = fetch_body_measures(conn, Some((from, to)))?;
    body_metrics.reverse();

    Ok(ReportData {
        sessions: session_details,
        exercises,
        body_metrics,
    })
}

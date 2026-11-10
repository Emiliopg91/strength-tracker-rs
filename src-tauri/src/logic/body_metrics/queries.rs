use rusqlite_orm::{
    dao::Repository,
    database::DatabasePool,
    errors::DatabaseError,
    types::{order_by::OrderBy, where_clause::Where},
};

use crate::{
    dao::body_metric::{self, BodyMetric, BodyMetricRepository},
    dto::body_metrics::BodyMetricListItem,
};

/// Lists all body measurements, newest first.
pub fn fetch_body_measures(
    conn: &rusqlite_orm::rusqlite::Connection,
    from_to: Option<(u32, u32)>,
) -> rusqlite_orm::errors::Result<Vec<BodyMetricListItem>> {
    let mut query =
        BodyMetricRepository::select().order_by(OrderBy::Desc(body_metric::entity::columns::DATE));

    if let Some((from, to)) = from_to {
        query = query.where_(Where::And(vec![
            Where::Gte(body_metric::entity::columns::DATE, from.into()),
            Where::Lte(body_metric::entity::columns::DATE, to.into()),
        ]))
    }

    Ok(query
        .fetch_in(conn)?
        .iter()
        .map(BodyMetricListItem::from)
        .collect())
}

/// Stores a new body measurement entry.
pub fn insert_body_metric(
    database: &DatabasePool,
    measures: &BodyMetricListItem,
) -> rusqlite_orm::errors::Result<()> {
    let mut item = BodyMetric::try_from(measures).map_err(DatabaseError::Transaction)?;
    BodyMetricRepository::insert()
        .item(&mut item)
        .execute(database)
        .map(|_| ())
}

/// Deletes the body measurement logged on `date`, if any.
pub fn delete_body_metric_on(
    database: &DatabasePool,
    date: u32,
) -> rusqlite_orm::errors::Result<()> {
    BodyMetricRepository::delete()
        .where_(Where::Eq(body_metric::entity::columns::DATE, date.into()))
        .execute(database)
        .map(|_| ())
}

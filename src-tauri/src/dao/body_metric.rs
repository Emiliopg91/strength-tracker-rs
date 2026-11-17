use std::collections::HashSet;

use rusqlite_orm::{
    Entity,
    dao::Repository,
    database::DatabasePool,
    errors::DatabaseError,
    rusqlite::{Connection, Transaction},
    types::{order_by::OrderBy, where_clause::Where},
};
use serde::{Deserialize, Serialize};

use crate::{connect::client::WeightEntry, utils::time::local_midnight};

#[derive(Entity, Serialize, Deserialize)]
#[entity("body_metric")]
#[primary_key(date)]
pub struct BodyMetric {
    pub date: u32,
    pub weight: f32,
    pub fat_ratio: f32,
    pub muscle_mass: f32,
    pub water_ratio: f32,
}

impl TryFrom<&WeightEntry> for BodyMetric {
    type Error = ();
    fn try_from(value: &WeightEntry) -> Result<Self, Self::Error> {
        Ok(BodyMetric {
            date: value
                .timestamp_gmt
                .and_then(|ms| local_midnight(ms.div_euclid(1000)))
                .ok_or(())?,
            weight: (value.weight.ok_or(())? as f32) / 1000_f32,
            // Weight-only entries (plain scales, manual input) lack body composition
            fat_ratio: value.body_fat.unwrap_or(0.0) as f32,
            muscle_mass: (value.muscle_mass.unwrap_or(0.0) as f32) / 1000_f32,
            water_ratio: value.body_water.unwrap_or(0.0) as f32,
        })
    }
}

impl BodyMetric {
    /// Lists all body measurements, newest first.
    pub async fn get_by_range(
        conn: &rusqlite_orm::rusqlite::Connection,
        from_to: Option<(u32, u32)>,
    ) -> rusqlite_orm::errors::Result<Vec<BodyMetric>> {
        let mut query =
            BodyMetricRepository::select().order_by(OrderBy::Desc(entity::columns::DATE));

        if let Some((from, to)) = from_to {
            query = query.where_(Where::And(vec![
                Where::Gte(entity::columns::DATE, from.into()),
                Where::Lte(entity::columns::DATE, to.into()),
            ]))
        }

        query.fetch_in(conn).await
    }

    /// Stores a new body measurement entry.
    pub async fn insert(
        database: &DatabasePool,
        item: &mut BodyMetric,
    ) -> rusqlite_orm::errors::Result<()> {
        BodyMetricRepository::insert()
            .item(item)
            .execute(database)
            .await
            .map(|_| ())
    }

    /// Deletes the body measurement logged on `date`, if any.
    pub async fn delete_on(database: &DatabasePool, date: u32) -> rusqlite_orm::errors::Result<()> {
        BodyMetricRepository::delete()
            .where_(Where::Eq(entity::columns::DATE, date.into()))
            .execute(database)
            .await
            .map(|_| ())
    }

    /// Dates of the stored body measurements, newest first.
    pub async fn get_dates(conn: &Connection) -> rusqlite_orm::errors::Result<Vec<u32>> {
        Ok(BodyMetricRepository::select()
            .columns(&[entity::columns::DATE])
            .order_by(OrderBy::Desc(entity::columns::DATE))
            .fetch_in(conn)
            .await?
            .iter()
            .map(|row| row.get_u32(entity::columns::DATE.as_ref()))
            .collect())
    }

    /// Stores the `entries` not logged yet (by date); returns how many were added.
    pub async fn insert_new(
        tx: &'_ Transaction<'_>,
        entries: &mut [BodyMetric],
        existing: &[u32],
    ) -> rusqlite_orm::errors::Result<usize> {
        let mut insert = BodyMetricRepository::insert().or_replace();
        for entry in entries {
            if !existing.contains(&entry.date) {
                insert = insert.item(entry);
            }
        }
        insert.execute_in(tx).await
    }
}

pub async fn migration_21(tx: &Transaction<'_>) -> Result<(), DatabaseError> {
    tx.execute_batch(
        "CREATE TEMP TABLE BODY_METRIC_OLD AS SELECT * FROM BODY_METRIC;
         DELETE FROM BODY_METRIC;",
    )
    .map_err(DatabaseError::Delete)?;

    let rows: Vec<(i64, u32)> = tx
        .prepare("SELECT rowid, date FROM BODY_METRIC_OLD ORDER BY date DESC")
        .and_then(|mut stmt| {
            stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
                .collect()
        })
        .map_err(DatabaseError::Select)?;

    let mut handled = HashSet::new();
    for (rowid, date) in rows {
        let date = local_midnight(date.into()).unwrap_or(date);
        if handled.insert(date) {
            tx.execute(
                "UPDATE BODY_METRIC_OLD SET date = ?1 WHERE rowid = ?2",
                (date, rowid),
            )
            .and_then(|_| {
                tx.execute(
                    "INSERT INTO BODY_METRIC SELECT * FROM BODY_METRIC_OLD WHERE rowid = ?1",
                    [rowid],
                )
            })
            .map_err(DatabaseError::Insert)?;
        }
    }

    tx.execute_batch("DROP TABLE BODY_METRIC_OLD;")
        .map_err(DatabaseError::Delete)?;

    Ok(())
}

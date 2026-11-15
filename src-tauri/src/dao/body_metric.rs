use std::collections::HashSet;

use rusqlite_orm::{Entity, errors::DatabaseError, rusqlite::Transaction};
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

/// Moves every body metric to its local midnight, keeping the latest one of each day.
///
/// Written against the raw table rather than the entity: later migrations rename its columns,
/// which don't exist yet when this one runs.
pub fn migration_21(tx: &Transaction) -> Result<(), DatabaseError> {
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

use std::collections::HashSet;

use rusqlite_orm::{
    Entity, dao::Repository, errors::DatabaseError, rusqlite::Transaction, types::order_by::OrderBy,
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
    pub lean_mass: f32,
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
            fat_ratio: value.body_fat.ok_or(())? as f32,
            lean_mass: (value.muscle_mass.ok_or(())? as f32) / 1000_f32,
            water_ratio: value.body_water.ok_or(())? as f32,
        })
    }
}

pub fn migration_21(tx: &Transaction) -> Result<(), DatabaseError> {
    let mut body_metrics = BodyMetricRepository::select()
        .order_by(OrderBy::Desc(entity::columns::DATE))
        .fetch_in(tx)?;
    BodyMetricRepository::delete().execute_in(tx)?;
    let mut handled = HashSet::new();
    for metric in &mut body_metrics {
        metric.date = local_midnight(metric.date.into()).unwrap_or(metric.date);
        if handled.insert(metric.date) {
            BodyMetricRepository::insert().item(metric).execute_in(tx)?;
        }
    }

    Ok(())
}

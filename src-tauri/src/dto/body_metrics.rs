use serde::{Deserialize, Serialize};

use crate::{dao::body_metric::BodyMetric, utils::time::local_midnight};

#[derive(Serialize, Deserialize)]
pub struct BodyMetricListItem {
    pub date: u32,
    pub weight: f32,
    pub fat_ratio: f32,
    pub lean_mass: f32,
    pub water_ratio: f32,
}

impl From<&BodyMetric> for BodyMetricListItem {
    fn from(value: &BodyMetric) -> Self {
        Self {
            date: value.date,
            weight: value.weight,
            fat_ratio: value.fat_ratio,
            lean_mass: value.lean_mass,
            water_ratio: value.water_ratio,
        }
    }
}

impl TryFrom<&BodyMetricListItem> for BodyMetric {
    type Error = Box<dyn std::error::Error + Send + Sync>;

    fn try_from(value: &BodyMetricListItem) -> Result<Self, Self::Error> {
        Ok(Self {
            date: local_midnight(value.date as i64).unwrap(),
            weight: value.weight,
            fat_ratio: value.fat_ratio,
            lean_mass: value.lean_mass,
            water_ratio: value.water_ratio,
        })
    }
}

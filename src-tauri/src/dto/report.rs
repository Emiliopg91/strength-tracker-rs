use serde::Serialize;

use crate::dto::{
    body_metrics::BodyMetricListItem, exercises::ExerciseDetails, sessions::SessionDetails,
};

#[derive(Serialize)]
pub struct ReportData {
    pub sessions: Vec<SessionDetails>,
    pub exercises: Vec<ExerciseDetails>,
    pub body_metrics: Vec<BodyMetricListItem>,
}

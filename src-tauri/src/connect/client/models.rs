use serde::{Deserialize, Serialize};

/// Activity summary returned from the activity list endpoint
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivitySummary {
    pub activity_id: u64,
    #[serde(default)]
    pub activity_name: Option<String>,
    /// Start time in local timezone (ISO 8601 format)
    #[serde(default)]
    pub start_time_local: Option<String>,
    /// Start time in GMT (ISO 8601 format)
    #[serde(default)]
    pub start_time_gmt: Option<String>,
    #[serde(default)]
    pub activity_type: Option<ActivityType>,
    /// Distance in meters
    #[serde(default)]
    pub distance: Option<f64>,
    /// Duration in seconds
    #[serde(default)]
    pub duration: Option<f64>,
    /// Elapsed duration in seconds (including pauses)
    #[serde(default)]
    pub elapsed_duration: Option<f64>,
    /// Moving duration in seconds
    #[serde(default)]
    pub moving_duration: Option<f64>,
    #[serde(default)]
    pub calories: Option<f64>,
    /// Average heart rate in bpm
    #[serde(default)]
    pub average_hr: Option<f64>,
    /// Maximum heart rate in bpm
    #[serde(default)]
    pub max_hr: Option<f64>,
    /// Average speed in m/s
    #[serde(default)]
    pub average_speed: Option<f64>,
    /// Maximum speed in m/s
    #[serde(default)]
    pub max_speed: Option<f64>,
    /// Total elevation gain in meters
    #[serde(default)]
    pub elevation_gain: Option<f64>,
    /// Total elevation loss in meters
    #[serde(default)]
    pub elevation_loss: Option<f64>,
    #[serde(default)]
    pub average_running_cadence_in_steps_per_minute: Option<f64>,
    #[serde(default)]
    pub steps: Option<u64>,
    #[serde(default)]
    pub has_polyline: Option<bool>,
    #[serde(default)]
    pub owner_display_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivityType {
    /// Type key (e.g., "running", "cycling", "strength_training")
    pub type_key: String,
    #[serde(default)]
    pub type_id: Option<u64>,
    #[serde(default)]
    pub parent_type_id: Option<u64>,
    #[serde(default)]
    pub is_hidden: Option<bool>,
}

/// Full activity details returned from the activity endpoint
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivityDetails {
    pub activity_id: u64,
    #[serde(default)]
    pub activity_name: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub start_time_local: Option<String>,
    #[serde(default)]
    pub start_time_gmt: Option<String>,
    #[serde(default)]
    pub activity_type: Option<ActivityType>,
    /// Summary metrics (distance, duration, etc.)
    #[serde(default)]
    pub summary_dto: Option<serde_json::Value>,
    #[serde(default)]
    pub location_name: Option<String>,
    #[serde(default)]
    pub time_zone_unit_dto: Option<serde_json::Value>,
    #[serde(default)]
    pub metadata_dto: Option<serde_json::Value>,
    /// Catch-all for unknown fields
    #[serde(flatten)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

/// Weight range response returned from the weight service
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WeightRange {
    #[serde(default)]
    pub date_weight_list: Vec<WeightEntry>,
}

/// Weight / body composition measurement
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WeightEntry {
    #[serde(default)]
    pub sample_pk: Option<u64>,
    /// Calendar date (YYYY-MM-DD)
    #[serde(default)]
    pub calendar_date: Option<String>,
    /// Measurement timestamp in local time (epoch milliseconds)
    #[serde(default)]
    pub date: Option<i64>,
    /// Measurement timestamp in GMT (epoch milliseconds)
    #[serde(default, rename = "timestampGMT")]
    pub timestamp_gmt: Option<i64>,
    /// Weight in grams
    #[serde(default)]
    pub weight: Option<f64>,
    #[serde(default)]
    pub bmi: Option<f64>,
    /// Body fat percentage
    #[serde(default)]
    pub body_fat: Option<f64>,
    /// Body water percentage
    #[serde(default)]
    pub body_water: Option<f64>,
    /// Bone mass in grams
    #[serde(default)]
    pub bone_mass: Option<f64>,
    /// Muscle mass in grams
    #[serde(default)]
    pub muscle_mass: Option<f64>,
    #[serde(default)]
    pub physique_rating: Option<f64>,
    #[serde(default)]
    pub visceral_fat: Option<f64>,
    #[serde(default)]
    pub metabolic_age: Option<f64>,
    #[serde(default)]
    pub source_type: Option<String>,
}

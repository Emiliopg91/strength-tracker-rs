use chrono::{Local, NaiveDate, NaiveDateTime, TimeZone};
use serde::{Deserialize, Deserializer, Serialize, de::Error};

/// Device entry returned from the device registration endpoint
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceSummary {
    pub display_name: String,
    pub unit_id: u64,
    pub workout_capable: bool,
    pub strength_workout_capable: bool,
}

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

/// Body of an activity update, which only changes the fields it carries
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivityDescriptionUpdate<'a> {
    pub activity_id: u64,
    pub description: &'a str,
}

/// Sets of a strength activity, as both read from and written back to the exercise sets endpoint
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ExerciseSets {
    pub activity_id: u64,
    pub exercise_sets: Vec<ExerciseSet>,
    /// Catch-all for unknown fields, sent back untouched on update
    #[serde(flatten)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

/// Kind of an exercise set. The set types are an open list, so unknown ones are kept as they came.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ExerciseSetType {
    Active,
    Rest,
    #[serde(untagged)]
    Other(String),
}

/// Set of a strength activity. The update replaces every set, so the fields not modelled here
/// (velocity, power and range of motion metrics) are kept in `extra` to be sent back as they came.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ExerciseSet {
    /// Exercise candidates, with the probability of each. A user pick is a single one at 100.
    #[serde(default)]
    pub exercises: Vec<ExerciseCandidate>,
    /// Seconds
    pub duration: Option<f64>,
    pub repetition_count: Option<u16>,
    /// Grams
    pub weight: Option<f64>,
    pub set_type: ExerciseSetType,
    /// Local time without zone, such as `2026-11-09T09:53:29.0`
    pub start_time: Option<String>,
    pub wkt_step_index: Option<u16>,
    /// Index of the `set` message in the FIT file
    pub message_index: Option<u16>,
    #[serde(flatten)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

/// Exercise a set may be of
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ExerciseCandidate {
    pub category: String,
    pub name: Option<String>,
    pub probability: f64,
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

/// Calendar month returned from the calendar service
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CalendarMonth {
    #[serde(default)]
    pub calendar_items: Vec<CalendarItem>,
}

/// Kind of a calendar item
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CalendarItemType {
    Activity,
    Weight,
    Nap,
    UserTimeChanged,
    #[serde(other)]
    Unknown,
}

/// Item of a calendar month
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CalendarItem {
    /// Missing on some item types, such as naps
    #[serde(default)]
    pub id: Option<u64>,
    pub item_type: CalendarItemType,
    /// Calendar day of the item
    pub date: NaiveDate,
    /// Start time in seconds since the epoch, reading Connect's local time in the system timezone.
    /// Only activities have it.
    #[serde(default, deserialize_with = "local_timestamp")]
    pub start_timestamp_local: Option<u32>,
}

/// Parses a zoneless date such as `2026-10-25T09:57:05.0`. A time skipped by a DST change gives `None`.
fn local_timestamp<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<u32>, D::Error> {
    let Some(date) = Option::<String>::deserialize(deserializer)? else {
        return Ok(None);
    };

    let date =
        NaiveDateTime::parse_from_str(&date, "%Y-%m-%dT%H:%M:%S%.f").map_err(D::Error::custom)?;
    Local
        .from_local_datetime(&date)
        .earliest()
        .map(|date| u32::try_from(date.timestamp()).map_err(D::Error::custom))
        .transpose()
}

/// Workout entry returned from the workout list and create endpoints
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkoutSummary {
    pub workout_id: u64,
    pub workout_name: String,
}

/// Entry of the device message queue, which Connect delivers on the device's next sync
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceMessage<'a> {
    pub device_id: u64,
    pub message_url: String,
    pub message_type: &'a str,
    pub message_name: &'a str,
    pub group_name: Option<&'a str>,
    pub priority: u8,
    pub file_type: &'a str,
    pub meta_data_id: u64,
}

use std::hash::Hash;

use serde::{Deserialize, Serialize};

use crate::{
    dao::{
        exercise::Exercise, session::Session, session_lap::SessionLap, session_step::SessionStep,
        workout_step::StepType,
    },
    dto::split_muscles,
};

#[derive(Serialize, Default)]
pub struct SessionListItem {
    pub name: String,
    pub timestamp: u32,
    pub active_calories: u16,
    pub training_load: u16,
    pub sport: u8,
    pub sub_sport: u8,
    pub has_record: bool,
    pub total_elapsed_time: u32,
    pub has_sets: bool,
}

impl From<&Session> for SessionListItem {
    fn from(value: &Session) -> Self {
        Self {
            name: value.name.clone(),
            timestamp: value.date,
            active_calories: value.active_calories,
            training_load: value.training_load,
            sport: value.sport,
            sub_sport: value.sub_sport,
            has_record: false,
            total_elapsed_time: value.total_elapsed_time,
            has_sets: false,
        }
    }
}
impl PartialEq for SessionListItem {
    fn eq(&self, other: &Self) -> bool {
        self.timestamp == other.timestamp
    }
}
impl Eq for SessionListItem {}
impl Hash for SessionListItem {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.timestamp.hash(state);
    }
}

#[derive(Serialize, Deserialize, Clone)]
pub struct SessionStepDto {
    pub ex_cat: Option<u16>,
    pub ex_id: Option<u16>,
    pub idx: u8,
    pub reps: Option<u16>,
    pub reps_ref: Option<u16>,
    pub weight: Option<f32>,
    pub weight_ref: Option<f32>,
    pub pr: bool,
    pub kind: StepType,
    pub duration: Option<u32>,
    pub duration_ref: Option<u32>,
}

impl From<&SessionStep> for SessionStepDto {
    fn from(value: &SessionStep) -> Self {
        Self {
            ex_cat: value.ex_cat,
            ex_id: value.ex_id,
            idx: value.idx,
            reps: value.reps,
            reps_ref: value.reps_ref,
            weight: value.weight,
            weight_ref: value.weight_ref,
            pr: value.pr,
            kind: value.kind.clone(),
            duration: value.duration,
            duration_ref: value.duration_ref,
        }
    }
}

#[derive(Serialize)]
pub struct SessionLapDto {
    idx: u16,
    start_latitude: Option<i32>,
    start_longitude: Option<i32>,
}

impl From<&SessionLap> for SessionLapDto {
    fn from(value: &SessionLap) -> Self {
        Self {
            idx: value.idx,
            start_latitude: value.start_latitude,
            start_longitude: value.start_longitude,
        }
    }
}

#[derive(Serialize)]
pub struct SessionDetails {
    pub name: String,

    pub timestamp: u32,

    pub total_elapsed_time: u32,
    pub active_time: u32,

    pub active_calories: u16,

    pub training_load: u16,
    pub sport: u8,
    pub sub_sport: u8,

    pub steps: Vec<SessionStepDto>,
    pub heart_rates: Vec<Option<u8>>,
    pub max_hr: u8,
    pub coordinates: Vec<Option<(i32, i32)>>,
    pub laps: Vec<SessionLapDto>,
    pub speeds: Vec<Option<f64>>,
    pub distance: Option<f64>,
    pub notes: String,

    pub primary_muscles: Vec<String>,
    pub secondary_muscles: Vec<String>,

    pub device: Option<String>,
    pub connect_id: Option<String>,
}

/// Rounds samples to `1 / scale` precision so their JSON form stays short (e.g. `scale = 1000.0` keeps 3 decimals).
fn round_samples(samples: Vec<Option<f64>>, scale: f64) -> Vec<Option<f64>> {
    samples
        .into_iter()
        .map(|v| v.map(|v| (v * scale).round() / scale))
        .collect()
}

/// Max heart rate assumed for sessions that don't record one.
const DEFAULT_MAX_HR: u8 = 189;

impl From<(&Session, &[Exercise], &[SessionStep], &[SessionLap])> for SessionDetails {
    fn from(
        (session, exercises, steps, laps): (&Session, &[Exercise], &[SessionStep], &[SessionLap]),
    ) -> Self {
        let data = session.data.as_ref();
        let (primary_muscles, secondary_muscles) = split_muscles(exercises);

        Self {
            name: session.name.clone(),
            timestamp: session.date,
            total_elapsed_time: session.total_elapsed_time,
            active_time: session.active_time,
            active_calories: session.active_calories,
            training_load: session.training_load,
            sport: session.sport,
            sub_sport: session.sub_sport,
            steps: steps.iter().map(SessionStepDto::from).collect(),
            heart_rates: data.and_then(|d| d.get_heart_rates()).unwrap_or_default(),
            max_hr: data.and_then(|d| d.max_hr).unwrap_or(DEFAULT_MAX_HR),
            device: session
                .device_obj
                .as_ref()
                .map(|dev| format!("Garmin {}", dev.model)),
            laps: laps.iter().map(SessionLapDto::from).collect(),
            coordinates: data
                .and_then(|d| d.get_coordinates_semicircle())
                .unwrap_or_default(),
            speeds: data
                .and_then(|d| d.get_speeds())
                .map(|s| round_samples(s, 1000.0))
                .unwrap_or_default(),
            distance: data.and_then(|d| d.distance).map(|d| d / 1000_f64),
            notes: data.and_then(|d| d.notes.clone()).unwrap_or_default(),
            primary_muscles,
            secondary_muscles,
            connect_id: session.connect_id.clone(),
        }
    }
}

#[derive(Deserialize, Serialize)]
pub struct SessionStepsUpdate {
    pub timestamp: u32,
    pub sets: Vec<SessionStepDto>,
    pub notes: Option<String>,
}

#[derive(Serialize, Clone)]
pub struct SessionLocation {
    pub session: u32,
    pub location: String,
}

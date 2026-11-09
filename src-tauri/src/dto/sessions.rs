use std::{collections::HashSet, hash::Hash};

use serde::{Deserialize, Serialize};

use crate::dao::{
    exercise::Exercise, session::Session, session_lap::SessionLap, session_step::SessionStep,
    workout_step::StepType,
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

impl From<(&Session, &[Exercise], &[SessionStep], &[SessionLap])> for SessionDetails {
    fn from(value: (&Session, &[Exercise], &[SessionStep], &[SessionLap])) -> Self {
        let device = value
            .0
            .device_obj
            .as_ref()
            .map(|dev| format!("Garmin {}", dev.model));

        let mut heart_rates = Vec::new();
        let mut max_hr = 189;
        let mut gps_coordinates = Vec::new();
        let mut speeds = Vec::new();
        let mut distance = None;
        let mut notes = "".to_string();

        if let Some(add_data) = &value.0.data {
            distance = add_data.distance.map(|d| d / 1000_f64);
            if let Some(hr_data) = add_data.get_heart_rates() {
                heart_rates = hr_data;
            }
            if let Some(coords) = add_data.get_coordinates_semicircle() {
                gps_coordinates = coords;
            }
            if let Some(spds) = add_data.get_speeds() {
                speeds = round_samples(spds, 1000.0);
            }
            if let Some(nts) = add_data.notes.clone() {
                notes = nts
            }
            if let Some(hr) = add_data.max_hr {
                max_hr = hr;
            }
        }

        let mut primary_set = HashSet::new();
        let mut secondary_set = HashSet::new();
        for exercise in value.1 {
            for muscle in &exercise.muscles {
                if muscle.is_primary {
                    primary_set.insert(muscle.muscle.clone());
                } else {
                    secondary_set.insert(muscle.muscle.clone());
                }
            }
        }

        let secondary_muscles = secondary_set
            .into_iter()
            .filter(|m| !primary_set.contains(m))
            .collect();
        let primary_muscles = primary_set.into_iter().collect();

        let sets = value.2.iter().map(SessionStepDto::from).collect();
        let laps = value.3.iter().map(SessionLapDto::from).collect();

        Self {
            name: value.0.name.clone(),
            timestamp: value.0.date,
            total_elapsed_time: value.0.total_elapsed_time,
            active_time: value.0.active_time,
            active_calories: value.0.active_calories,
            training_load: value.0.training_load,
            sport: value.0.sport,
            sub_sport: value.0.sub_sport,
            steps: sets,
            heart_rates,
            max_hr,
            device,
            laps,
            coordinates: gps_coordinates,
            speeds,
            distance,
            notes,
            primary_muscles,
            secondary_muscles,
            connect_id: value.0.connect_id.clone(),
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

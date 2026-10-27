use std::{collections::HashSet, hash::Hash};

use geo_types::Point;
use gpx::{Gpx, Track, TrackSegment, Waypoint};
use serde::{Deserialize, Serialize};

use crate::{
    dao::{exercise::Exercise, lap::Lap, session::Session, set::Set},
    utils::constants,
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

#[derive(Serialize, Deserialize)]
pub struct SessionSet {
    pub ex_cat: u16,
    pub ex_id: u16,
    pub idx: u8,
    pub reps: u16,
    pub weight: f32,
    pub pr: bool,
    pub rest: bool,
    pub duration: u32,
}

impl From<&Set> for SessionSet {
    fn from(value: &Set) -> Self {
        Self {
            ex_cat: value.ex_cat,
            ex_id: value.ex_id,
            idx: value.idx,
            reps: value.reps,
            weight: value.weight,
            pr: value.pr,
            rest: value.ex_cat == u16::MAX,
            duration: value.duration,
        }
    }
}

#[derive(Serialize)]
pub struct SessionLap {
    idx: u16,
    start_latitude: Option<i32>,
    start_longitude: Option<i32>,
}

impl From<&Lap> for SessionLap {
    fn from(value: &Lap) -> Self {
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

    pub sets: Vec<SessionSet>,
    pub heart_rates: Vec<Option<u8>>,
    pub max_hr: u8,
    pub coordinates: Vec<Option<(i32, i32)>>,
    pub altitudes: Vec<Option<f64>>,
    pub laps: Vec<SessionLap>,
    pub speeds: Vec<Option<f64>>,
    pub distance: Option<f64>,
    pub notes: String,

    pub primary_muscles: Vec<String>,
    pub secondary_muscles: Vec<String>,

    pub device: Option<String>,
}

/// Rounds samples to `1 / scale` precision so their JSON form stays short (e.g. `scale = 1000.0` keeps 3 decimals).
fn round_samples(samples: Vec<Option<f64>>, scale: f64) -> Vec<Option<f64>> {
    samples
        .into_iter()
        .map(|v| v.map(|v| (v * scale).round() / scale))
        .collect()
}

impl From<(&Session, &[Exercise], &[Set], &[Lap])> for SessionDetails {
    fn from(value: (&Session, &[Exercise], &[Set], &[Lap])) -> Self {
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
        let mut altitudes = Vec::new();
        let mut notes = "".to_string();

        if let Some(add_data) = &value.0.additional_data {
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
            if let Some(alts) = add_data.get_altitudes() {
                altitudes = round_samples(alts, 10.0)
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

        let sets = value.2.iter().map(SessionSet::from).collect();
        let laps = value.3.iter().map(SessionLap::from).collect();

        Self {
            name: value.0.name.clone(),
            timestamp: value.0.date,
            total_elapsed_time: value.0.total_elapsed_time,
            active_time: value.0.active_time,
            active_calories: value.0.active_calories,
            training_load: value.0.training_load,
            sport: value.0.sport,
            sub_sport: value.0.sub_sport,
            sets,
            heart_rates,
            max_hr,
            device,
            laps,
            coordinates: gps_coordinates,
            speeds,
            distance,
            altitudes,
            notes,
            primary_muscles,
            secondary_muscles,
        }
    }
}

#[derive(Deserialize, Serialize)]
pub struct SessionSetsUpdate {
    pub timestamp: u32,
    pub sets: Vec<SessionSet>,
    pub notes: Option<String>,
}

#[derive(Serialize, Clone)]
pub struct SessionLocation {
    pub session: u32,
    pub location: String,
}

impl From<Session> for Gpx {
    fn from(session: Session) -> Self {
        let mut gpx = Gpx::default();

        gpx.version = gpx::GpxVersion::Gpx11;
        gpx.creator = Some(constants::APP_TITLE.to_string());

        let mut track = Track::new();
        track.name = Some(session.name);

        let mut segment = TrackSegment::new();
        if let Some(additional_data) = &session.additional_data
            && let Some(coords) = additional_data.get_coordinates_degrees()
        {
            let altitudes = additional_data.get_altitudes().unwrap_or_default();

            for (idx, coord) in coords.into_iter().enumerate() {
                if let Some((lat, lon)) = coord {
                    let mut wpt = Waypoint::new(Point::new(lon, lat));
                    if let Some(altitude) = altitudes.get(idx) {
                        wpt.elevation = *altitude;
                    }
                    segment.points.push(wpt);
                }
            }
        }

        track.segments.push(segment);
        gpx.tracks.push(track);

        for lap in session.laps {
            if let Some((lat, lon)) = lap.get_coordinates_degrees() {
                let mut wp = Waypoint::new(Point::new(lon, lat));
                wp.name = Some(format!("{}", lap.idx + 1));
                gpx.waypoints.push(wp);
            }
        }

        gpx
    }
}

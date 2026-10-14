use std::collections::{HashMap, HashSet};

use rusqlite_orm::{dao::Repository, database::DatabasePool};
use serde::Serialize;

use crate::{
    dao::{
        additional_data::{AdditionalData, AdditionalDataRepository},
        body_metric::{BodyMetric, BodyMetricRepository},
        device::{Device, DeviceRepository},
        lap::{Lap, LapRepository},
        session::{Session, SessionRepository},
        set::{Set, SetRepository},
        settings::{Settings, SettingsRepository},
        workout::{Workout, WorkoutRepository},
    },
    dto::sessions::SessionSet,
    utils::translations::{Languages, translate},
};

#[derive(Serialize)]
pub struct Export {
    body_metrics: Vec<BodyMetric>,
    exercises: Vec<ExerciseExport>,
    devices: Vec<Device>,
    workouts: Vec<Workout>,
    sessions: Vec<SessionExport>,
    settings: Vec<Settings>,
    laps: Vec<LapExport>,
}

impl Export {
    /// Loads every table from the database and assembles a full export snapshot.
    pub fn from_database(db: &DatabasePool, lang: Languages) -> rusqlite_orm::errors::Result<Self> {
        db.run_in_connection(|conn| {
            let body_metrics = BodyMetricRepository::select().fetch_in(conn)?;
            let mut workouts = WorkoutRepository::select().fetch_in(conn)?;
            for workout in &mut workouts {
                workout.fetch_steps_relationship_in(conn)?;
            }
            let devices = DeviceRepository::select().fetch_in(conn)?;
            let settings = SettingsRepository::select().fetch_in(conn)?;
            let sessions = SessionRepository::select().fetch_in(conn)?;
            let laps = LapRepository::select()
                .fetch_in(conn)?
                .into_iter()
                .map(LapExport::from)
                .collect::<Vec<_>>();

            let mut additional_datas: HashMap<u32, AdditionalData> = HashMap::new();
            AdditionalDataRepository::select()
                .fetch_in(conn)?
                .into_iter()
                .for_each(|ad| {
                    additional_datas.insert(ad.session, ad);
                });

            let mut used_exercises = HashSet::new();

            let mut series: HashMap<u32, Vec<Set>> = HashMap::new();
            SetRepository::select()
                .fetch_in(conn)?
                .into_iter()
                .for_each(|s| {
                    used_exercises.insert((s.ex_cat, s.ex_id));
                    let entry = series.entry(s.session).or_default();
                    entry.push(s);
                });

            let sessions = sessions
                .into_iter()
                .map(|session| {
                    let serie = series.get(&session.date);
                    let add_data = additional_datas.get(&session.date);

                    SessionExport::from((&session, add_data, serie))
                })
                .collect::<Vec<_>>();

            let exercises = used_exercises
                .into_iter()
                .map(|ue| ExerciseExport {
                    category: ue.0,
                    id: ue.1,
                    name: translate(&format!("exercise_{}_{}", ue.0, ue.1), lang),
                })
                .collect();

            Ok(Self {
                body_metrics,
                devices,
                workouts,
                sessions,
                exercises,
                settings,
                laps,
            })
        })
    }

    /// Serializes the export snapshot as JSON straight into `writer`, avoiding a full in-memory copy.
    pub fn write_json<W: std::io::Write>(&self, writer: W) -> Result<(), serde_json::Error> {
        serde_json::to_writer(writer, &self)
    }
}

#[derive(Serialize)]
pub struct SessionExport {
    pub date: u32,
    pub workout: String,
    pub total_elapsed_time: u32,
    pub active_time: u32,
    pub active_calories: u16,
    pub training_load: u16,
    pub sport: u8,
    pub sub_sport: u8,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub coordinates: Option<Vec<Option<(f64, f64)>>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub speeds: Option<Vec<Option<f64>>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub altitudes: Option<Vec<Option<f64>>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub heart_rates: Option<Vec<Option<u8>>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sets: Option<Vec<SessionSet>>,
}

impl From<(&Session, Option<&AdditionalData>, Option<&Vec<Set>>)> for SessionExport {
    fn from(values: (&Session, Option<&AdditionalData>, Option<&Vec<Set>>)) -> Self {
        let mut heart_rates = None;
        let mut coordinates: Option<Vec<Option<(f64, f64)>>> = None;
        let mut speeds: Option<Vec<Option<f64>>> = None;
        let mut series = None;
        let mut altitudes = None;

        if let Some(add_data) = values.1 {
            heart_rates = add_data.get_heart_rates();
            coordinates = add_data.get_coordinates_degrees();
            speeds = add_data.get_speeds();
            altitudes = add_data.get_altitudes();
        }

        if let Some(srs) = values.2
            && !srs.is_empty()
        {
            let sers = srs.iter().map(SessionSet::from).collect();

            series = Some(sers);
        }

        Self {
            date: values.0.date,
            workout: values.0.name.clone(),
            total_elapsed_time: values.0.total_elapsed_time,
            active_time: values.0.active_time,
            active_calories: values.0.active_calories,
            training_load: values.0.training_load,
            sport: values.0.sport,
            sub_sport: values.0.sub_sport,
            device: values.0.device.clone(),
            sets: series,
            heart_rates,
            coordinates,
            speeds,
            altitudes,
        }
    }
}

#[derive(Serialize)]
pub struct ExerciseExport {
    category: u16,
    id: u16,
    name: String,
}

#[derive(Serialize)]
pub struct LapExport {
    pub session: u32,
    pub idx: u16,
    pub start_latitude: Option<f64>,
    pub start_longitude: Option<f64>,
}

impl From<Lap> for LapExport {
    fn from(value: Lap) -> Self {
        Self {
            session: value.session,
            idx: value.idx,
            start_latitude: value
                .start_latitude
                .map(|s| s as f64 * AdditionalData::SEMICIRCLE_TO_DEGREES),
            start_longitude: value
                .start_longitude
                .map(|s| s as f64 * AdditionalData::SEMICIRCLE_TO_DEGREES),
        }
    }
}

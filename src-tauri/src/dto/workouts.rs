use serde::Serialize;

use crate::dao::{session::Session, workout_step::WorkoutStep};

#[derive(Serialize)]
pub struct WorkoutListItem {
    pub name: String,
    pub latest_session: Option<u32>,
    pub sessions: u32,
    pub avg_time: u32,
    pub avg_load: u16,
    pub enabled: bool,
    pub has_steps: bool,
}

#[derive(Serialize)]
pub struct WorkoutSession {
    pub date: u32,
    pub volume: f32,
    pub time: u32,
}

impl From<&Session> for WorkoutSession {
    fn from(value: &Session) -> Self {
        WorkoutSession {
            date: value.date,
            volume: 0_f32,
            time: value.total_elapsed_time,
        }
    }
}

#[derive(Serialize)]
pub struct WorkoutDetails {
    pub name: String,
    pub latest_session: Option<u32>,
    pub session_count: u32,
    pub avg_time: u32,
    pub avg_volume: f32,
    pub sessions: Vec<WorkoutSession>,
    pub enabled: bool,
    pub notes: Option<String>,
    pub steps: Vec<WorkoutStep>,
}

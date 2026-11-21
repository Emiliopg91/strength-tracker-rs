use serde::Serialize;

use crate::dao::{session::Session, workout_planning::WorkoutPlanning, workout_step::WorkoutStep};

#[derive(Serialize)]
pub struct WorkoutListItem {
    pub id: u16,
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
    pub id: u16,
    pub name: String,
    pub latest_session: Option<u32>,
    pub session_count: u32,
    pub avg_time: u32,
    pub avg_volume: f32,
    pub sessions: Vec<WorkoutSession>,
    pub enabled: bool,
    pub progression: bool,
    pub notes: Option<String>,
    pub steps: Vec<WorkoutStep>,
    pub primary_muscles: Vec<String>,
    pub secondary_muscles: Vec<String>,
}

/// What a step loaded with a percentage of the 1RM resolves against for an exercise.
#[derive(Serialize)]
pub struct PercentLoadBase {
    pub ex_cat: u16,
    pub ex_id: u16,
    /// Best estimated 1RM of the recent sets, in kg
    pub one_rm: f32,
}

#[derive(Serialize)]
pub struct WorkoutPlanListItem {
    pub date: u32,
    pub workout: u16,
    pub session: Option<u32>,
}

impl From<&WorkoutPlanning> for WorkoutPlanListItem {
    fn from(value: &WorkoutPlanning) -> Self {
        Self {
            date: value.date,
            workout: value.workout,
            session: value.session,
        }
    }
}

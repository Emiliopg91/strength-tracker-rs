use rusqlite_orm::Entity;
use serde::{Deserialize, Serialize};

use crate::dao::{
    exercise::{self},
    workout_step::StepType,
};

use super::exercise::Exercise;

#[derive(Entity, Clone, Serialize, Deserialize)]
#[entity("session_step")]
#[primary_key(session, idx)]
#[index("session", (session))]
#[index("personal_records", (pr))]
#[index("session_pr", (session, pr))]
#[index("exercise", (ex_cat, ex_id))]
#[unique("exercise_personal_record", (ex_cat, ex_id), (pr=true))]
pub struct SessionStep {
    pub session: u32,
    pub idx: u8,
    #[column("exercise_category")]
    pub ex_cat: Option<u16>,
    #[column("exercise_id")]
    pub ex_id: Option<u16>,
    pub reps: Option<u16>,
    pub reps_ref: Option<u16>,
    pub weight: Option<f32>,
    pub weight_ref: Option<f32>,
    pub pr: bool,
    pub duration: Option<u32>,
    pub duration_ref: Option<u32>,
    pub kind: StepType,

    #[relationship((ex_cat, exercise::entity::columns::CATEGORY),(ex_id, exercise::entity::columns::ID))]
    #[allow(dead_code)]
    pub exercise: Option<Exercise>,
}

use rusqlite_orm::Entity;
use serde::{Deserialize, Serialize};

use crate::dao::exercise::{self};

use super::exercise::Exercise;

#[derive(Entity, Clone, Serialize, Deserialize)]
#[primary_key(session, idx)]
#[index("session", (session))]
#[index("personal_records", (pr))]
#[index("session_pr", (session, pr))]
#[index("exercise", (ex_cat, ex_id))]
#[unique("exercise_personal_record", (ex_cat, ex_id), (pr=true))]
pub struct Set {
    pub session: u32,
    pub idx: u8,
    #[column("exercise_category")]
    pub ex_cat: u16,
    #[column("exercise_id")]
    pub ex_id: u16,
    pub reps: u16,
    pub weight: f32,
    pub pr: bool,
    pub duration: u32,

    #[relationship((ex_cat, exercise::entity::columns::CATEGORY),(ex_id, exercise::entity::columns::ID))]
    #[allow(dead_code)]
    pub exercise: Option<Exercise>,
}

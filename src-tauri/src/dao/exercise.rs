use rusqlite_orm::Entity;
use serde::{Deserialize, Serialize};

use crate::dao::exercise_category::{self, ExerciseCategory};

#[derive(Clone, Entity, Serialize, Deserialize)]
#[entity(hashable = true, comparable = true)]
#[primary_key(category, id)]
pub struct Exercise {
    pub category: u16,
    pub id: u16,
    pub unilateral: bool,
    pub progression: bool,
    pub reps: String,
    pub weight: f32,

    #[relationship((category, exercise_category::entity::columns::ID))]
    pub exercise_category: Option<ExerciseCategory>,
}

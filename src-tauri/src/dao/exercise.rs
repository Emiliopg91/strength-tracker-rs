use rusqlite_orm::Entity;
use serde::{Deserialize, Serialize};

use crate::dao::{
    exercise_category::{self, ExerciseCategory},
    muscle_exercise::{self, MuscleExercise},
};

/// Id of the generic exercise of each category: what FIT files carry as an invalid exercise name.
pub const GENERIC_EXERCISE_ID: u16 = u16::MAX;

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
    pub notes: Option<String>,
    pub name: String,

    #[relationship((category, exercise_category::entity::columns::ID))]
    pub exercise_category: Option<ExerciseCategory>,

    #[relationship((category, muscle_exercise::entity::columns::EX_CAT),(id, muscle_exercise::entity::columns::EX_ID))]
    pub muscles: Vec<MuscleExercise>,
}

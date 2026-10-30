use rusqlite_orm::Entity;
use serde::{Deserialize, Serialize};

#[derive(Clone, Entity, Serialize, Deserialize)]
#[entity("exercise_category")]
#[primary_key(id)]
pub struct ExerciseCategory {
    pub id: u16,
    pub name: String,
}

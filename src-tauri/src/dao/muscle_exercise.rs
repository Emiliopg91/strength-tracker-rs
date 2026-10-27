use rusqlite_orm::Entity;
use serde::{Deserialize, Serialize};

use crate::dao::muscle::{self, Muscle};

#[derive(Entity, Clone, Deserialize, Serialize)]
#[entity("muscle_exercise")]
pub struct MuscleExercise {
    pub muscle: String,
    #[column("exercise_category")]
    pub ex_cat: u16,
    #[column("exercise_id")]
    pub ex_id: u16,
    pub is_primary: bool,

    #[relationship((muscle, muscle::entity::columns::NAME))]
    #[allow(dead_code)]
    pub muscle_obj: Option<Muscle>,
}

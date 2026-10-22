use rusqlite_orm::Entity;
use serde::{Deserialize, Serialize};

use crate::dao::workout_step::{self, WorkoutStep};

#[derive(Entity, Clone, Serialize, Deserialize)]
#[primary_key(name)]
pub struct Workout {
    pub name: String,
    pub suggest: bool,
    pub notes: Option<String>,

    #[relationship((name, workout_step::entity::columns::WORKOUT))]
    pub steps: Vec<WorkoutStep>,
}

impl Workout {
    pub fn get_workout_file_name(&self) -> String {
        let name = &self.name[..self.name.floor_char_boundary(31)];
        format!("{}_workout.fit", name.replace(' ', "_"))
    }
}

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::{dao::exercise::Exercise, dto::sessions::SessionSet};

#[derive(Serialize)]
pub struct ExerciseListItem {
    pub category: u16,
    pub id: u16,
    pub reps: u16,
    pub weight: f32,
    pub date: u32,
    pub unilateral: bool,
    pub progression: bool,
}

impl From<&Exercise> for ExerciseListItem {
    fn from(value: &Exercise) -> Self {
        Self {
            category: value.category,
            id: value.id,
            reps: 0,
            weight: 0_f32,
            date: 0,
            unilateral: value.unilateral,
            progression: false,
        }
    }
}

#[derive(Serialize)]
pub struct ExerciseDetails {
    pub category: u16,
    pub id: u16,
    pub reps: u16,
    pub weight: f32,
    pub unilateral: bool,
    pub workouts: Vec<String>,
    pub series: HashMap<String, Vec<SessionSet>>,
    pub pr_date: u32,
    pub progression: bool,
    pub reps_range: String,
    pub prog_weight: f32,
}

impl From<&Exercise> for ExerciseDetails {
    fn from(value: &Exercise) -> Self {
        Self {
            category: value.category,
            id: value.id,
            reps: 0,
            weight: 0_f32,
            workouts: Vec::new(),
            series: HashMap::new(),
            pr_date: 0,
            unilateral: value.unilateral,
            progression: value.progression,
            reps_range: value.reps.clone(),
            prog_weight: value.weight,
        }
    }
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct ProgressionSuggestion {
    pub weight: f32,
    pub reps: u16,
}

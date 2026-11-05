use rusqlite_orm::{
    Entity,
    rusqlite::{
        ToSql,
        types::{FromSql, FromSqlError, FromSqlResult, ToSqlOutput, ValueRef},
    },
    types::value::Value,
};
use rustyfit::profile::typedef::Intensity;
use serde::{Deserialize, Serialize};

use crate::dao::exercise::{self, Exercise};

#[derive(Clone, Serialize, Deserialize, Entity)]
#[entity("workout_step")]
#[primary_key(workout, idx)]
#[index("workout",(workout))]
#[index("exercise",(ex_cat, ex_id))]
pub struct WorkoutStep {
    #[serde(skip)]
    pub workout: String,
    pub idx: u16,
    pub kind: StepType,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[column("exercise_category")]
    pub ex_cat: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[column("exercise_id")]
    pub ex_id: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reps: Option<Option<u16>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub weight: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// In seconds; FIT files carry it in milliseconds
    pub time: Option<Option<u32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub begin_idx: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,

    #[relationship((ex_cat, exercise::entity::columns::CATEGORY),(ex_id, exercise::entity::columns::ID))]
    pub exercise: Option<Exercise>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum StepType {
    Exercise,
    Warmup,
    Recovery,
    Cooldown,
    Rest,
    Other,
    Repeat,
}

impl ToSql for StepType {
    fn to_sql(&self) -> rusqlite_orm::rusqlite::Result<ToSqlOutput<'_>> {
        Ok(ToSqlOutput::from(match self {
            StepType::Exercise => "Exercise",
            StepType::Warmup => "Warmup",
            StepType::Recovery => "Recovery",
            StepType::Cooldown => "Cooldown",
            StepType::Rest => "Rest",
            StepType::Repeat => "Repeat",
            StepType::Other => "Other",
        }))
    }
}

impl FromSql for StepType {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        match value.as_str()? {
            "Exercise" => Ok(StepType::Exercise),
            "Warmup" => Ok(StepType::Warmup),
            "Recovery" => Ok(StepType::Recovery),
            "Cooldown" => Ok(StepType::Cooldown),
            "Rest" => Ok(StepType::Rest),
            "Repeat" => Ok(StepType::Repeat),
            "Other" => Ok(StepType::Other),
            other => Err(FromSqlError::Other(
                format!("Invalid StepType '{other}'").into(),
            )),
        }
    }
}

impl From<StepType> for Value {
    fn from(value: StepType) -> Self {
        Value::Text(
            match value {
                StepType::Exercise => "Exercise",
                StepType::Warmup => "Warmup",
                StepType::Recovery => "Recovery",
                StepType::Cooldown => "Cooldown",
                StepType::Rest => "Rest",
                StepType::Repeat => "Repeat",
                StepType::Other => "Other",
            }
            .to_string(),
        )
    }
}

impl From<Intensity> for StepType {
    fn from(value: Intensity) -> Self {
        match value {
            Intensity::ACTIVE => StepType::Exercise,
            Intensity::WARMUP => StepType::Warmup,
            Intensity::RECOVERY => StepType::Recovery,
            Intensity::COOLDOWN => StepType::Cooldown,
            Intensity::REST => StepType::Rest,
            Intensity::OTHER | _ => StepType::Other,
        }
    }
}

impl WorkoutStep {
    pub fn new(workout: &str, idx: u16, kind: StepType) -> Self {
        Self {
            workout: workout.to_string(),
            idx,
            kind,
            begin_idx: None,
            ex_cat: None,
            ex_id: None,
            reps: None,
            time: None,
            weight: None,
            notes: None,
            exercise: None,
        }
    }
    pub fn rest(workout: &str, idx: u16, time: Option<u32>, notes: Option<String>) -> Self {
        let mut inst = Self::new(workout, idx, StepType::Rest);
        inst.time = Some(time);
        inst.notes = notes;
        inst
    }

    pub fn repeat(workout: &str, idx: u16, from: u16, reps: u16) -> Self {
        let mut inst = Self::new(workout, idx, StepType::Repeat);
        inst.begin_idx = Some(from);
        inst.reps = Some(Some(reps));
        inst
    }

    #[allow(clippy::too_many_arguments)]
    pub fn active(
        workout: &str,
        idx: u16,
        kind: Intensity,
        exercise: Option<(u16, u16)>,
        weight: f32,
        reps: Option<u16>,
        time: Option<u32>,
        notes: Option<String>,
    ) -> Self {
        let mut inst = Self::new(workout, idx, StepType::Exercise);
        inst.kind = kind.into();
        inst.ex_cat = exercise.map(|(cat, _)| cat);
        inst.ex_id = exercise.map(|(_, id)| id);
        inst.weight = Some(weight);
        inst.reps = Some(reps);
        inst.time = Some(time);
        inst.notes = notes;
        inst
    }
}

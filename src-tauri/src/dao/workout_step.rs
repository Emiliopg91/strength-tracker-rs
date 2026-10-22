use rusqlite_orm::{
    Entity,
    rusqlite::{
        ToSql,
        types::{FromSql, FromSqlError, FromSqlResult, ToSqlOutput, ValueRef},
    },
    types::value::Value,
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, Entity)]
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
    pub time: Option<Option<u32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub begin_idx: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum StepType {
    Exercise,
    Rest,
    Repeat,
}

impl ToSql for StepType {
    fn to_sql(&self) -> rusqlite_orm::rusqlite::Result<ToSqlOutput<'_>> {
        Ok(ToSqlOutput::from(match self {
            StepType::Exercise => "Exercise",
            StepType::Rest => "Rest",
            StepType::Repeat => "Repeat",
        }))
    }
}

impl FromSql for StepType {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        match value.as_str()? {
            "Exercise" => Ok(StepType::Exercise),
            "Rest" => Ok(StepType::Rest),
            "Repeat" => Ok(StepType::Repeat),
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
                StepType::Rest => "Rest",
                StepType::Repeat => "Repeat",
            }
            .to_string(),
        )
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
    pub fn exercise(
        workout: &str,
        idx: u16,
        ex_cat: u16,
        ex_id: u16,
        weight: f32,
        reps: Option<u16>,
        time: Option<u32>,
        notes: Option<String>,
    ) -> Self {
        let mut inst = Self::new(workout, idx, StepType::Exercise);
        inst.ex_cat = Some(ex_cat);
        inst.ex_id = Some(ex_id);
        inst.weight = Some(weight);
        inst.reps = Some(reps);
        inst.time = Some(time);
        inst.notes = notes;
        inst
    }
}

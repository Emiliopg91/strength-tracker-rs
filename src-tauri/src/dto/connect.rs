use std::ops::Range;

use serde::{Deserialize, Serialize};

use crate::dao::{
    settings,
    workout::Workout,
    workout_step::{self},
};

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectWorkout {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workout_id: Option<u64>,
    pub workout_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub sport_type: SportType,
    pub workout_segments: Vec<WorkoutSegment>,
}

impl ConnectWorkout {
    pub fn from(value: &Workout, weight_unit: settings::WeightUnit) -> Self {
        ConnectWorkout {
            workout_id: value.connect_id.as_ref().map(|v| v.parse().unwrap()),
            workout_name: value.name.clone(),
            description: value.notes.clone(),
            sport_type: SportType::default(),
            workout_segments: vec![WorkoutSegment {
                workout_steps: WorkoutStep::from_workout(&value.steps, weight_unit),
                ..Default::default()
            }],
        }
    }
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SportType {
    pub sport_type_id: u8,
    pub sport_type_key: String,
}
impl Default for SportType {
    fn default() -> Self {
        Self {
            sport_type_id: 5,
            sport_type_key: "strength_training".to_string(),
        }
    }
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkoutSegment {
    pub segment_order: u8,
    pub sport_type: SportType,
    pub workout_steps: Vec<WorkoutStep>,
}

impl Default for WorkoutSegment {
    fn default() -> Self {
        Self {
            segment_order: 1,
            sport_type: SportType::default(),
            workout_steps: vec![],
        }
    }
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum WorkoutStep {
    #[serde(rename = "ExecutableStepDTO")]
    Executable(ExecutableStep),
    #[serde(rename = "RepeatGroupDTO")]
    RepeatGroup(RepeatGroup),
}

impl WorkoutStep {
    pub fn from_workout(
        steps: &[workout_step::WorkoutStep],
        weight_unit: settings::WeightUnit,
    ) -> Vec<Self> {
        let mut res = Self::from_range(steps, 0..steps.len(), weight_unit);
        Self::assign_step_order(&mut res, &mut 1, &mut 1, None);
        res
    }

    /// Connect numbers steps 1-based and depth-first, with each repeat group before its children.
    /// Each repeat group gets its own 1-based `childStepId`, which its direct children repeat.
    fn assign_step_order(
        steps: &mut [Self],
        next: &mut u16,
        next_child: &mut u16,
        parent: Option<u16>,
    ) {
        for step in steps {
            match step {
                WorkoutStep::Executable(step) => {
                    step.step_order = *next;
                    step.child_step_id = parent;
                    *next += 1;
                }
                WorkoutStep::RepeatGroup(group) => {
                    group.step_order = *next;
                    group.child_step_id = *next_child;
                    *next += 1;
                    *next_child += 1;
                    Self::assign_step_order(
                        &mut group.workout_steps,
                        next,
                        next_child,
                        Some(group.child_step_id),
                    );
                }
            }
        }
    }

    /// `range` and `begin_idx` are absolute indices into `steps`, so nested repeats slice correctly.
    /// `step_order` and `child_step_id` are left unset and filled afterwards by `assign_step_order`.
    fn from_range(
        steps: &[workout_step::WorkoutStep],
        range: Range<usize>,
        weight_unit: settings::WeightUnit,
    ) -> Vec<Self> {
        let mut res: Vec<WorkoutStep> = Vec::new();

        let mut end = range.end;
        while end > range.start {
            let idx = end - 1;
            let step = &steps[idx];
            match step.kind {
                workout_step::StepType::Rest => res.push(WorkoutStep::Executable(ExecutableStep {
                    step_order: 0,
                    child_step_id: None,
                    step_type: StepType::rest(),
                    end_condition: if step.time.flatten().is_some() {
                        EndCondition::time()
                    } else {
                        EndCondition::lap_button()
                    },
                    end_condition_value: if let Some(time) = step.time.flatten() {
                        Some((time / 1000) as f64)
                    } else {
                        None
                    },
                    description: step.notes.clone(),
                    category: None,
                    exercise_name: None,
                    weight_unit: None,
                    weight_value: None,
                })),
                workout_step::StepType::Exercise => {
                    res.push(WorkoutStep::Executable(ExecutableStep {
                        step_order: 0,
                        child_step_id: None,
                        step_type: StepType::interval(),
                        end_condition: if step.reps.flatten().is_some() {
                            EndCondition::reps()
                        } else {
                            if step.time.flatten().is_some() {
                                EndCondition::time()
                            } else {
                                EndCondition::lap_button()
                            }
                        },
                        end_condition_value: if let Some(reps) = step.reps.flatten() {
                            Some(reps as f64)
                        } else {
                            if let Some(time) = step.time.flatten() {
                                Some((time / 1000) as f64)
                            } else {
                                None
                            }
                        },
                        description: step.notes.clone(),
                        category: Some(
                            step.exercise
                                .clone()
                                .unwrap()
                                .exercise_category
                                .unwrap()
                                .name,
                        ),
                        exercise_name: Some(step.exercise.clone().unwrap().name),
                        weight_unit: if step.weight.is_some() {
                            match weight_unit {
                                settings::WeightUnit::Kilograms => Some(WeightUnit::kilogram()),
                                settings::WeightUnit::Pounds => Some(WeightUnit::pound()),
                            }
                        } else {
                            None
                        },
                        weight_value: if let Some(weight) = step.weight.clone() {
                            Some(weight as f64)
                        } else {
                            None
                        },
                    }))
                }
                workout_step::StepType::Repeat => {
                    let begin = step.begin_idx.unwrap() as usize;
                    res.push(WorkoutStep::RepeatGroup(RepeatGroup {
                        step_order: 0,
                        child_step_id: 0,
                        step_type: StepType::repeat(),
                        end_condition: EndCondition::iterations(),
                        end_condition_value: step.reps.flatten().unwrap() as f64,
                        number_of_iterations: step.reps.flatten().unwrap(),
                        smart_repeat: false,
                        workout_steps: WorkoutStep::from_range(steps, begin..idx, weight_unit),
                    }));
                    end = begin;
                    continue;
                }
            }

            end -= 1;
        }

        res.reverse();

        res
    }
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecutableStep {
    pub step_order: u16,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub child_step_id: Option<u16>,
    pub step_type: StepType,
    pub end_condition: EndCondition,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_condition_value: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exercise_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub weight_value: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub weight_unit: Option<WeightUnit>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RepeatGroup {
    pub step_order: u16,
    pub child_step_id: u16,
    pub step_type: StepType,
    pub number_of_iterations: u16,
    pub end_condition: EndCondition,
    pub end_condition_value: f64,
    pub smart_repeat: bool,
    pub workout_steps: Vec<WorkoutStep>,
}

// StepType, EndCondition, WeightUnit: same as now + derives + rename_all = "camelCase"
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StepType {
    pub step_type_id: u8,
    pub step_type_key: String,
}

impl StepType {
    pub fn interval() -> Self {
        Self {
            step_type_id: 3,
            step_type_key: "interval".to_string(),
        }
    }
    pub fn rest() -> Self {
        Self {
            step_type_id: 5,
            step_type_key: "rest".to_string(),
        }
    }
    pub fn repeat() -> Self {
        Self {
            step_type_id: 6,
            step_type_key: "repeat".to_string(),
        }
    }
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EndCondition {
    pub condition_type_id: u8,
    pub condition_type_key: String,
}

impl EndCondition {
    pub fn reps() -> Self {
        Self {
            condition_type_id: 10,
            condition_type_key: "reps".to_string(),
        }
    }
    pub fn time() -> Self {
        Self {
            condition_type_id: 2,
            condition_type_key: "time".to_string(),
        }
    }
    pub fn iterations() -> Self {
        Self {
            condition_type_id: 7,
            condition_type_key: "iterations".to_string(),
        }
    }
    pub fn lap_button() -> Self {
        Self {
            condition_type_id: 1,
            condition_type_key: "lap.button".to_string(),
        }
    }
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WeightUnit {
    pub unit_id: u8,
    pub unit_key: String,
    pub factor: f64,
}

impl WeightUnit {
    pub fn pound() -> Self {
        Self {
            unit_id: 9,
            unit_key: "pound".to_string(),
            factor: 453.59237,
        }
    }
    pub fn kilogram() -> Self {
        Self {
            unit_id: 8,
            unit_key: "kilogram".to_string(),
            factor: 1000.0,
        }
    }
}

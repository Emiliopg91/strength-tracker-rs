use std::collections::HashSet;

use rustyfit::{
    profile::{
        mesgdef::{ExerciseTitle, Workout as FitWorkout, WorkoutStep as FitWorkoutStep},
        typedef::{
            self, ExerciseCategory, FitBaseUnit, Intensity, MessageIndex, Sport, SubSport,
            WktStepDuration, WktStepTarget, WorkoutCapabilities,
        },
    },
    proto::Message,
};

use crate::{
    dao::{
        settings::WeightUnit,
        workout::Workout,
        workout_step::{StepType, WorkoutStep},
    },
    fit::writer::ToFitMessages,
    utils::translations::{Languages, translate},
};

/// Cuts `s` to at most `max` bytes on a char boundary to keep valid UTF-8
fn truncate_utf8(s: &str, max: usize) -> String {
    s[..s.floor_char_boundary(max)].to_string()
}

/// Inverse of the mapping done in `TryFrom<FitParser> for Workout`.
fn step_message(step: &WorkoutStep, weight_unit: WeightUnit) -> Message {
    let mut msg = FitWorkoutStep::new();
    msg.message_index = MessageIndex(step.idx);
    msg.target_type = WktStepTarget::OPEN;

    match step.kind {
        StepType::Exercise => {
            msg.intensity = Intensity::ACTIVE;
            if let Some(Some(time)) = step.time {
                msg.duration_type = WktStepDuration::TIME;
                msg.duration_value = time;
            } else {
                match step.reps.flatten() {
                    Some(reps) => {
                        msg.duration_type = WktStepDuration::REPS;
                        msg.duration_value = reps as u32;
                    }
                    None => msg.duration_type = WktStepDuration::OPEN,
                }
            }
            if let Some(cat) = step.ex_cat {
                msg.exercise_category = ExerciseCategory(cat);
            }
            if let Some(id) = step.ex_id {
                msg.exercise_name = id;
            }

            msg.weight_display_unit = match weight_unit {
                WeightUnit::Kilograms => FitBaseUnit::KILOGRAM,
                WeightUnit::Pounds => FitBaseUnit::POUND,
            };
            msg.notes = truncate_utf8(step.notes.as_deref().unwrap_or_default(), 200).into();

            if let Some(weight) = step.weight {
                msg.set_exercise_weight_scaled(weight as f64);
            }
        }
        StepType::Rest => {
            msg.intensity = Intensity::REST;
            msg.secondary_target_type = WktStepTarget::OPEN;
            msg.weight_display_unit = match weight_unit {
                WeightUnit::Kilograms => FitBaseUnit::KILOGRAM,
                WeightUnit::Pounds => FitBaseUnit::POUND,
            };
            msg.notes = truncate_utf8(step.notes.as_deref().unwrap_or_default(), 200).into();

            match step.time {
                Some(Some(time)) => {
                    msg.duration_type = WktStepDuration::TIME;
                    msg.duration_value = time;
                }
                _ => msg.duration_type = WktStepDuration::OPEN,
            }
        }
        StepType::Repeat => {
            msg.duration_type = WktStepDuration::REPEAT_UNTIL_STEPS_CMPLT;
            msg.duration_value = step.begin_idx.unwrap() as u32;
            msg.target_value = step.reps.flatten().unwrap() as u32;
        }
    }

    msg.into()
}

impl ToFitMessages for Workout {
    fn file_type(&self) -> typedef::File {
        typedef::File::WORKOUT
    }

    fn file_key(&self) -> u32 {
        // Without a Connect id, derive a stable serial from the name above the ids Connect has handed out so far
        self.connect_id
            .as_deref()
            .and_then(|v| v.parse().ok())
            .unwrap_or_else(|| {
                const BASE: u32 = 1_746_147_457;
                // FNV-1a, stable across builds
                let hash = self.name.bytes().fold(0x811c_9dc5_u32, |h, b| {
                    (h ^ b as u32).wrapping_mul(0x0100_0193)
                });
                BASE + 1 + hash % (u32::MAX - BASE)
            })
    }

    fn to_fit_messages(
        &self,
        lang: Languages,
        weight_unit: WeightUnit,
    ) -> super::errors::Result<Vec<Message>> {
        let mut workout = FitWorkout::new();
        workout.capabilities = WorkoutCapabilities::TCX;
        workout.sport = Sport::TRAINING;
        workout.sub_sport = SubSport::STRENGTH_TRAINING;
        workout.num_valid_steps = self.steps.len() as u16;
        // Garmin caps wkt_name at 32 bytes; cut on a char boundary to keep valid UTF-8
        workout.wkt_name = truncate_utf8(&self.name, 32).into();
        workout.wkt_description =
            truncate_utf8(self.notes.as_deref().unwrap_or_default(), 200).into();

        let mut steps: Vec<&WorkoutStep> = self.steps.iter().collect();
        steps.sort_by_key(|s| s.idx);

        let workout_steps = steps
            .iter()
            .copied()
            .map(|step| step_message(step, weight_unit));

        let mut idx = 0;
        let mut seen = HashSet::new();
        let titles = steps.iter().filter_map(|step| {
            let mut msg = None;

            if let Some(ex_cat) = step.ex_cat
                && let Some(ex_id) = step.ex_id
                && seen.insert((ex_cat, ex_id))
            {
                let mut ex_title = ExerciseTitle::new();
                ex_title.exercise_category = ExerciseCategory(ex_cat);
                ex_title.exercise_name = ex_id;
                ex_title.wkt_step_name =
                    vec![translate(&format!("exercise_{}_{}", ex_cat, ex_id), lang).into()].into();
                ex_title.message_index = MessageIndex(idx);

                idx += 1;

                msg = Some(ex_title.into())
            }

            msg
        });

        let mut messages = vec![workout.into()];
        messages.extend(workout_steps);
        messages.extend(titles);

        Ok(messages)
    }
}

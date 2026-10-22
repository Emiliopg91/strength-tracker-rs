use crate::{
    dao::{workout::Workout, workout_step::WorkoutStep},
    fit::parser::{
        FitParser,
        errors::{self, ParseFitFileError},
    },
};
use rustyfit::{
    DecoderEvent, StreamingIterator,
    profile::{
        mesgdef,
        typedef::{Intensity, MesgNum, Sport, SubSport, WktStepDuration},
    },
};

impl TryFrom<FitParser> for Workout {
    type Error = errors::ParseFitFileError;
    fn try_from(mut value: FitParser) -> Result<Self, Self::Error> {
        let path_string = value.borrow_path().display().to_string();
        let mut workout = WorkoutAccumulator::default();

        value.with_stream_mut(|stream| -> errors::Result<()> {
            while let Some(event) = stream.next() {
                let event = event.map_err(|e| {
                    ParseFitFileError::FileReading(path_string.clone(), Box::new(e))
                })?;
                if let DecoderEvent::Message(msg) = event {
                    match msg.num {
                        MesgNum::WORKOUT => {
                            workout.set_workout(&mesgdef::Workout::from(msg))?;
                        }
                        MesgNum::WORKOUT_STEP => {
                            workout.push_step(&mesgdef::WorkoutStep::from(msg));
                        }
                        _ => (),
                    }
                }
            }

            Ok(())
        })?;

        workout.build()
    }
}

/// Accumulates the `workout` and `workout_step` FIT messages as they stream in, so both the
/// workout and the session parsers can build a [`Workout`] from them.
#[derive(Default)]
pub(super) struct WorkoutAccumulator {
    name: String,
    notes: Option<String>,
    /// `None` until the `workout` message is seen, then whether it is a strength workout
    is_strength: Option<bool>,
    steps: Vec<WorkoutStep>,
}

impl WorkoutAccumulator {
    pub(super) fn set_workout(&mut self, msg: &mesgdef::Workout) -> errors::Result<()> {
        let is_strength =
            msg.sport == Sport::TRAINING && msg.sub_sport == SubSport::STRENGTH_TRAINING;
        self.is_strength = Some(is_strength);
        if !is_strength {
            return Err(ParseFitFileError::NotAWorkout());
        }

        self.name = msg.wkt_name.to_string();
        self.notes = if !msg.wkt_description.is_empty() {
            Some(msg.wkt_description.to_string())
        } else {
            None
        };

        Ok(())
    }

    pub(super) fn push_step(&mut self, msg: &mesgdef::WorkoutStep) {
        let name = &self.name;
        let idx = msg.message_index.0;
        match msg.duration_type {
            WktStepDuration::REPEAT_UNTIL_STEPS_CMPLT => {
                self.steps.push(WorkoutStep::repeat(
                    name,
                    idx,
                    msg.duration_value as u16,
                    msg.target_value as u16,
                ));
            }
            _ => match msg.intensity {
                Intensity::REST => {
                    let duration = match msg.duration_type {
                        WktStepDuration::TIME => Some(msg.duration_value),
                        _ => None,
                    };
                    let notes = if !msg.notes.is_empty() {
                        Some(msg.notes.to_string())
                    } else {
                        None
                    };
                    self.steps
                        .push(WorkoutStep::rest(name, idx, duration, notes));
                }
                Intensity::ACTIVE => {
                    let ex_cat = msg.exercise_category.0;
                    let ex_id = if msg.exercise_name == u16::MAX {
                        1
                    } else {
                        msg.exercise_name
                    };
                    let weight = msg.exercise_weight_scaled().unwrap_or(0_f64) as f32;
                    let reps = match msg.duration_type {
                        WktStepDuration::REPS => Some(msg.duration_value as u16),
                        _ => None,
                    };
                    let time = match msg.duration_type {
                        WktStepDuration::TIME => Some(msg.duration_value),
                        _ => None,
                    };
                    let notes = if !msg.notes.is_empty() {
                        Some(msg.notes.to_string())
                    } else {
                        None
                    };
                    self.steps.push(WorkoutStep::exercise(
                        name, idx, ex_cat, ex_id, weight, reps, time, notes,
                    ));
                }
                _ => {}
            },
        }
    }

    pub(super) fn build(mut self) -> errors::Result<Workout> {
        match self.is_strength {
            None => return Err(ParseFitFileError::MissingField("name".to_string())),
            Some(false) => return Err(ParseFitFileError::NotAWorkout()),
            Some(true) => {}
        }
        if self.name.is_empty() {
            return Err(ParseFitFileError::MissingField("name".to_string()));
        }

        self.steps.sort_by_key(|s| s.idx);
        let old_idxs: Vec<u16> = self.steps.iter().map(|s| s.idx).collect();

        for (new_idx, step) in self.steps.iter_mut().enumerate() {
            step.idx = new_idx as u16;
            if let Some(begin) = step.begin_idx {
                step.begin_idx = Some(old_idxs.partition_point(|&old| old < begin) as u16);
            }
        }

        Ok(Workout {
            name: self.name,
            suggest: true,
            steps: self.steps,
            notes: self.notes,
        })
    }
}

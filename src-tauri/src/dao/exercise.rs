use std::collections::{BTreeSet, HashMap};

use rusqlite_orm::{
    Entity,
    dao::Repository,
    rusqlite::{Connection, Transaction},
    types::where_clause::Where,
};
use serde::{Deserialize, Serialize};

use crate::dao::{
    exercise_category::{self, ExerciseCategory},
    muscle_exercise::{
        self, MuscleExercise, MuscleExerciseRepository, entity::columns as muscle_exercise_cols,
    },
    session_step::{
        SessionStepRepository, entity::columns as session_step_cols, exercise_key_values,
    },
    workout::{WorkoutRepository, entity::columns as workout_cols},
    workout_device::{WorkoutDeviceRepository, entity::columns as workout_device_cols},
    workout_step::{StepType, WorkoutStepRepository, entity::columns as workout_step_cols},
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

impl Exercise {
    /// The exercises with the given `(category, id)` keys, with their category filled, by key, in
    /// two queries.
    pub async fn get_by_keys_with_category<'a>(
        conn: &Connection,
        keys: impl IntoIterator<Item = &'a (u16, u16)>,
    ) -> rusqlite_orm::errors::Result<HashMap<(u16, u16), Exercise>> {
        let mut exercises: HashMap<(u16, u16), Exercise> = Exercise::get_by_keys(conn, keys)
            .await?
            .into_iter()
            .map(|e| ((e.category, e.id), e))
            .collect();

        let category_ids: BTreeSet<u16> = exercises.keys().map(|(cat, _)| *cat).collect();
        let categories: HashMap<u16, ExerciseCategory> =
            ExerciseCategory::get_by_ids(conn, &category_ids)
                .await?
                .into_iter()
                .map(|c| (c.id, c))
                .collect();
        for exercise in exercises.values_mut() {
            exercise.exercise_category = categories.get(&exercise.category).cloned();
        }
        Ok(exercises)
    }

    /// Notes of the exercises with the given `(category, id)` keys, for those that have any.
    pub async fn get_notes<'a>(
        conn: &Connection,
        keys: impl IntoIterator<Item = &'a (u16, u16)>,
    ) -> rusqlite_orm::errors::Result<HashMap<(u16, u16), String>> {
        Ok(Exercise::get_by_keys(conn, keys)
            .await?
            .into_iter()
            .filter_map(|e| Some(((e.category, e.id), e.notes?)))
            .collect())
    }

    /// The exercise, `None` if it isn't in the catalog.
    pub async fn get(
        conn: &Connection,
        category: u16,
        id: u16,
    ) -> rusqlite_orm::errors::Result<Option<Exercise>> {
        ExerciseRepository::select_by_id_in(conn, category, id).await
    }

    /// The exercise with its muscles.
    ///
    /// # Panics
    /// If the exercise isn't in the catalog.
    pub async fn get_with_muscles(
        conn: &Connection,
        category: u16,
        id: u16,
    ) -> rusqlite_orm::errors::Result<Exercise> {
        let mut exercise = ExerciseRepository::select_by_id_in(conn, category, id)
            .await?
            .unwrap();
        exercise.fetch_muscles_relationship_in(conn).await?;
        Ok(exercise)
    }

    /// Every exercise in the catalog.
    pub async fn get_all(conn: &Connection) -> rusqlite_orm::errors::Result<Vec<Exercise>> {
        ExerciseRepository::select().fetch_in(conn).await
    }

    /// The exercises with the given `(category, id)` keys, unordered.
    pub async fn get_by_keys<'a>(
        conn: &Connection,
        keys: impl IntoIterator<Item = &'a (u16, u16)>,
    ) -> rusqlite_orm::errors::Result<Vec<Exercise>> {
        let keys = exercise_key_values(keys);
        if keys.is_empty() {
            return Ok(Vec::new());
        }
        ExerciseRepository::select()
            .where_(Where::InMultiple(
                vec![entity::columns::CATEGORY, entity::columns::ID],
                keys,
            ))
            .fetch_in(conn)
            .await
    }

    /// The exercises programmed in the steps of the workout `name`.
    pub async fn get_by_workout(
        conn: &Connection,
        workout: u16,
    ) -> rusqlite_orm::errors::Result<Vec<Exercise>> {
        ExerciseRepository::select()
            .where_(Where::InMultipleSub(
                vec![entity::columns::CATEGORY, entity::columns::ID],
                WorkoutStepRepository::select()
                    .distinct(&[workout_step_cols::EX_CAT, workout_step_cols::EX_ID])
                    .where_(Where::Eq(workout_step_cols::WORKOUT, workout.into()))
                    .to_subquery(),
            ))
            .fetch_in(conn)
            .await
    }

    /// Fills the muscles of every exercise in a single query, instead of one per exercise as the
    /// relationship fetcher would.
    pub async fn fill_muscles(
        conn: &Connection,
        exercises: &mut [Exercise],
    ) -> rusqlite_orm::errors::Result<()> {
        if exercises.is_empty() {
            return Ok(());
        }

        let mut by_exercise: HashMap<(u16, u16), Vec<MuscleExercise>> = HashMap::new();
        for muscle in MuscleExerciseRepository::select()
            .where_(Where::InMultiple(
                vec![muscle_exercise_cols::EX_CAT, muscle_exercise_cols::EX_ID],
                exercise_key_values(exercises.iter().map(|e| (e.category, e.id))),
            ))
            .fetch_in(conn)
            .await?
        {
            by_exercise
                .entry((muscle.ex_cat, muscle.ex_id))
                .or_default()
                .push(muscle);
        }

        for exercise in exercises {
            exercise.muscles = by_exercise
                .remove(&(exercise.category, exercise.id))
                .unwrap_or_default();
        }
        Ok(())
    }

    /// Exercises with progression enabled that could have a suggestion: performed at least once
    /// and programmed in an enabled workout with progression. Prunes the catalog in SQL so the per-exercise checks
    /// only run for the handful of exercises actually in use.
    pub async fn get_progression_candidates(
        conn: &Connection,
    ) -> rusqlite_orm::errors::Result<Vec<Exercise>> {
        let performed = SessionStepRepository::select()
            .distinct(&[session_step_cols::EX_CAT, session_step_cols::EX_ID])
            .where_(Where::Eq(
                session_step_cols::KIND,
                StepType::Exercise.into(),
            ))
            .to_subquery();
        let programmed = WorkoutStepRepository::select()
            .distinct(&[workout_step_cols::EX_CAT, workout_step_cols::EX_ID])
            .where_(Where::And(vec![
                Where::Eq(workout_step_cols::KIND, StepType::Exercise.into()),
                Where::InSub(
                    workout_step_cols::WORKOUT,
                    WorkoutRepository::select()
                        .columns(&[workout_cols::ID])
                        .where_(Where::And(vec![
                            Where::Eq(workout_cols::ENABLED, true.into()),
                            Where::Eq(workout_cols::PROGRESSION, true.into()),
                        ]))
                        .to_subquery(),
                ),
            ]))
            .to_subquery();
        let key = || vec![entity::columns::CATEGORY, entity::columns::ID];
        ExerciseRepository::select()
            .where_(Where::And(vec![
                Where::Eq(entity::columns::PROGRESSION, true.into()),
                Where::InMultipleSub(key(), performed),
                Where::InMultipleSub(key(), programmed),
            ]))
            .fetch_in(conn)
            .await
    }

    /// Marks the exercise as performed one side at a time, or not.
    pub async fn set_unilateral(
        conn: &Connection,
        category: u16,
        id: u16,
        value: bool,
    ) -> rusqlite_orm::errors::Result<()> {
        ExerciseRepository::update()
            .set(entity::columns::UNILATERAL, value.into())
            .where_(by_key(category, id))
            .execute_in(conn)
            .await?;
        Ok(())
    }

    /// Enables or disables progression suggestions for the exercise.
    pub async fn set_progression(
        conn: &Connection,
        category: u16,
        id: u16,
        value: bool,
    ) -> rusqlite_orm::errors::Result<()> {
        ExerciseRepository::update()
            .set(entity::columns::PROGRESSION, value.into())
            .where_(by_key(category, id))
            .execute_in(conn)
            .await?;
        Ok(())
    }

    /// Stores the rep range (e.g. `8-10-12`) and weight increment used to suggest progressions.
    pub async fn set_progression_parameters(
        conn: &Connection,
        category: u16,
        id: u16,
        reps: String,
        weight: f32,
    ) -> rusqlite_orm::errors::Result<()> {
        ExerciseRepository::update()
            .set(entity::columns::REPS, reps.into())
            .set(entity::columns::WEIGHT, weight.into())
            .where_(by_key(category, id))
            .execute_in(conn)
            .await?;
        Ok(())
    }

    /// Stores the exercise notes and queues for upload the workouts inheriting them, those with
    /// progression. Returns every workout inheriting the notes.
    pub async fn update_notes(
        tx: &Transaction<'_>,
        category: u16,
        id: u16,
        notes: Option<String>,
    ) -> rusqlite_orm::errors::Result<Vec<u16>> {
        ExerciseRepository::update()
            .set(entity::columns::NOTES, notes.into())
            .where_(by_key(category, id))
            .execute_in(tx)
            .await?;

        let workouts: Vec<u16> = WorkoutStepRepository::select()
            .distinct(&[workout_step_cols::WORKOUT])
            .where_(Where::And(vec![
                Where::Eq(workout_step_cols::EX_CAT, category.into()),
                Where::Eq(workout_step_cols::EX_ID, id.into()),
                Where::Or(vec![
                    Where::Null(workout_step_cols::NOTES),
                    Where::Eq(workout_step_cols::NOTES, "".into()),
                ]),
                Where::InSub(
                    workout_step_cols::WORKOUT,
                    WorkoutRepository::select()
                        .columns(&[workout_cols::ID])
                        .where_(Where::Eq(workout_cols::PROGRESSION, true.into()))
                        .to_subquery(),
                ),
            ]))
            .fetch_in(tx)
            .await?
            .iter()
            .map(|r| r.get_u16(workout_step_cols::WORKOUT.as_ref()))
            .collect();

        if !workouts.is_empty() {
            // On a disabled workout `pending` means deletion from the device; enabling it queues
            // the upload
            WorkoutDeviceRepository::update()
                .set(workout_device_cols::PENDING, true.into())
                .where_(Where::And(vec![
                    Where::In(
                        workout_device_cols::WORKOUT,
                        workouts.iter().map(|w| (*w).into()).collect(),
                    ),
                    Where::InSub(
                        workout_device_cols::WORKOUT,
                        WorkoutRepository::select()
                            .distinct(&[workout_cols::ID])
                            .where_(Where::Eq(workout_cols::ENABLED, true.into()))
                            .to_subquery(),
                    ),
                ]))
                .execute_in(tx)
                .await?;
        }

        Ok(workouts)
    }
}

/// Condition matching the exercise with that key.
fn by_key(category: u16, id: u16) -> Where<Exercise> {
    Where::And(vec![
        Where::Eq(entity::columns::CATEGORY, category.into()),
        Where::Eq(entity::columns::ID, id.into()),
    ])
}

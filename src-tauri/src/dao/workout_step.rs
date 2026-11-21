use std::collections::{BTreeSet, HashMap, HashSet};

use rusqlite_orm::{
    Entity,
    dao::Repository,
    rusqlite::{
        Connection, ToSql, Transaction,
        types::{FromSql, FromSqlError, FromSqlResult, ToSqlOutput, ValueRef},
    },
    types::{order_by::OrderBy, value::Value, where_clause::Where},
};
use rustyfit::profile::typedef::Intensity;
use serde::{Deserialize, Serialize};

use crate::{
    dao::{
        exercise::{self, Exercise},
        session_step::{SessionStep, exercise_key_values},
        workout::{WorkoutRepository, entity::columns as workout_cols},
        workout_device::{WorkoutDeviceRepository, entity::columns as workout_device_cols},
    },
    utils::strength::{estimate_1rm, percent_load},
};

/// Weeks of sets the current estimated 1RM of an exercise is taken from.
pub const ONE_RM_WINDOW_WEEKS: u32 = 8;

#[derive(Clone, Serialize, Deserialize, Entity)]
#[entity("workout_step")]
#[primary_key(workout, idx)]
#[index("workout",(workout))]
#[index("exercise",(ex_cat, ex_id))]
pub struct WorkoutStep {
    #[serde(skip)]
    pub workout: u16,
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
    /// In kg. With `one_rm_pct`, the load that percentage resolves to, `None` without a 1RM
    pub weight: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Percentage of the current estimated 1RM of the exercise the step is loaded with
    pub one_rm_pct: Option<u8>,
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

impl WorkoutStep {
    /// Fills the exercise, with its category, of every exercise step in two queries, instead of
    /// two per step as the relationship fetchers would.
    pub async fn fill_exercises(
        conn: &Connection,
        steps: &mut [WorkoutStep],
    ) -> rusqlite_orm::errors::Result<()> {
        let keys: BTreeSet<(u16, u16)> =
            steps.iter().filter_map(|s| s.ex_cat.zip(s.ex_id)).collect();
        if keys.is_empty() {
            return Ok(());
        }
        let exercises = Exercise::get_by_keys_with_category(conn, &keys).await?;

        for step in steps {
            step.exercise = step
                .ex_cat
                .zip(step.ex_id)
                .and_then(|key| exercises.get(&key).cloned());
        }
        Ok(())
    }

    /// Inserts the steps, each carrying its workout.
    pub async fn insert_all(
        tx: &Transaction<'_>,
        steps: &mut [WorkoutStep],
    ) -> rusqlite_orm::errors::Result<usize> {
        let mut insert = WorkoutStepRepository::insert();
        for step in steps {
            insert = insert.item(step);
        }
        insert.execute_in(tx).await
    }

    /// Keys of every exercise programmed in some workout step.
    pub async fn get_planned_exercises(
        conn: &Connection,
    ) -> rusqlite_orm::errors::Result<HashSet<(u16, u16)>> {
        Ok(WorkoutStepRepository::select()
            .distinct(&[entity::columns::EX_CAT, entity::columns::EX_ID])
            .where_(Where::And(vec![
                Where::NotNull(entity::columns::EX_CAT),
                Where::NotNull(entity::columns::EX_ID),
            ]))
            .fetch_in(conn)
            .await?
            .iter()
            .filter_map(|r| {
                r.get_u16_nullable(entity::columns::EX_CAT.as_ref())
                    .zip(r.get_u16_nullable(entity::columns::EX_ID.as_ref()))
            })
            .collect())
    }

    /// Ids of the workouts with some step stored.
    pub async fn get_workouts_with_steps(
        conn: &Connection,
    ) -> rusqlite_orm::errors::Result<HashSet<u16>> {
        Ok(WorkoutStepRepository::select()
            .distinct(&[entity::columns::WORKOUT])
            .fetch_in(conn)
            .await?
            .iter()
            .map(|r| r.get_u16(entity::columns::WORKOUT.as_ref()))
            .collect())
    }

    /// Loads programmed with reps for the exercises in `keys` in enabled workouts with progression,
    /// as `(exercise, weight, reps)`, a missing weight counting as 0. Steps of other workouts are
    /// left out.
    pub async fn get_programmed_loads(
        conn: &Connection,
        keys: &HashSet<(u16, u16)>,
    ) -> rusqlite_orm::errors::Result<Vec<((u16, u16), f32, u16)>> {
        Ok(WorkoutStepRepository::select()
            .columns(&[
                entity::columns::EX_CAT,
                entity::columns::EX_ID,
                entity::columns::WEIGHT,
                entity::columns::REPS,
            ])
            .where_(Where::And(vec![
                Where::Eq(entity::columns::KIND, StepType::Exercise.into()),
                Where::NotNull(entity::columns::REPS),
                // Percentage loads grow with the 1RM, so they stay out of the progression
                Where::Null(entity::columns::ONE_RM_PCT),
                Where::InMultiple(
                    vec![entity::columns::EX_CAT, entity::columns::EX_ID],
                    exercise_key_values(keys),
                ),
                in_progression_workouts(true),
            ]))
            .fetch_in(conn)
            .await?
            .iter()
            .map(|row| {
                (
                    (
                        row.get_u16(entity::columns::EX_CAT.as_ref()),
                        row.get_u16(entity::columns::EX_ID.as_ref()),
                    ),
                    row.get_f32_nullable(entity::columns::WEIGHT.as_ref())
                        .unwrap_or_default(),
                    row.get_u16(entity::columns::REPS.as_ref()),
                )
            })
            .collect())
    }

    /// Highest load programmed for the exercise across the workouts in `workouts_with_prog`, as
    /// weight and reps, a missing value counting as 0.
    pub async fn get_highest_load(
        conn: &Connection,
        category: u16,
        id: u16,
        workouts_with_prog: &[u16],
    ) -> rusqlite_orm::errors::Result<Option<(f32, u16)>> {
        Ok(WorkoutStepRepository::select()
            .columns(&[entity::columns::REPS, entity::columns::WEIGHT])
            .where_(Where::And(vec![
                Where::Eq(entity::columns::EX_CAT, category.into()),
                Where::Eq(entity::columns::EX_ID, id.into()),
                Where::Eq(entity::columns::KIND, StepType::Exercise.into()),
                Where::Null(entity::columns::ONE_RM_PCT),
                Where::In(
                    entity::columns::WORKOUT,
                    workouts_with_prog.iter().map(|&w| w.into()).collect(),
                ),
            ]))
            .order_by(OrderBy::Desc(entity::columns::WEIGHT))
            .order_by(OrderBy::Desc(entity::columns::REPS))
            .fetch_one_in(conn)
            .await?
            .map(|r| {
                (
                    r.get_f32_nullable(entity::columns::WEIGHT.as_ref())
                        .unwrap_or_default(),
                    r.get_u16_nullable(entity::columns::REPS.as_ref())
                        .unwrap_or_default(),
                )
            }))
    }

    /// Raises the exercise steps programmed below `weight` x `reps` in workouts with progression and
    /// queues those workouts for the devices. Returns whether any step changed and the workouts
    /// with progression programming the exercise.
    pub async fn apply_progression(
        tx: &Transaction<'_>,
        category: u16,
        id: u16,
        weight: f32,
        reps: u16,
    ) -> rusqlite_orm::errors::Result<(bool, Vec<u16>)> {
        let of_exercise = || {
            vec![
                Where::Eq(entity::columns::EX_CAT, category.into()),
                Where::Eq(entity::columns::EX_ID, id.into()),
                Where::Eq(entity::columns::KIND, StepType::Exercise.into()),
                Where::Null(entity::columns::ONE_RM_PCT),
                in_progression_workouts(false),
            ]
        };

        // Reps go first: the weight update would otherwise change which steps are below
        let mut conditions = of_exercise();
        conditions.push(Where::NotNull(entity::columns::REPS));
        conditions.push(below_load(weight, reps));
        let mut modified = WorkoutStepRepository::update()
            .set(entity::columns::REPS, reps.into())
            .where_(Where::And(conditions))
            .execute_in(tx)
            .await?;

        let mut conditions = of_exercise();
        conditions.push(weight_below(weight));
        modified += WorkoutStepRepository::update()
            .set(entity::columns::WEIGHT, weight.into())
            .where_(Where::And(conditions))
            .execute_in(tx)
            .await?;

        let mut workouts = Vec::new();
        if modified > 0 {
            workouts = WorkoutStepRepository::select()
                .distinct(&[entity::columns::WORKOUT])
                .where_(Where::And(of_exercise()))
                .fetch_in(tx)
                .await?
                .iter()
                .map(|r| r.get_u16(entity::columns::WORKOUT.as_ref()))
                .collect();
            WorkoutDeviceRepository::update()
                .set(workout_device_cols::PENDING, true.into())
                .where_(Where::InSub(
                    workout_device_cols::WORKOUT,
                    WorkoutStepRepository::select()
                        .distinct(&[entity::columns::WORKOUT])
                        .where_(Where::And(of_exercise()))
                        .to_subquery(),
                ))
                .execute_in(tx)
                .await?;
        }

        Ok((modified > 0, workouts))
    }

    /// Resolves the steps loaded with a percentage of the 1RM, of the exercises in `exercises` or
    /// of every exercise with `None`, against the best estimated 1RM of the sets performed in the
    /// [`ONE_RM_WINDOW_WEEKS`] before `now`, rounded to the weight increment of the exercise. A
    /// step without sets to estimate from goes unloaded. Workouts with a changed load are queued
    /// for the devices; returns their ids.
    pub async fn refresh_percent_loads(
        tx: &Transaction<'_>,
        exercises: Option<&HashSet<(u16, u16)>>,
        now: u32,
    ) -> rusqlite_orm::errors::Result<Vec<u16>> {
        let mut conditions = vec![
            Where::NotEq(entity::columns::KIND, StepType::Rest.into()),
            Where::NotNull(entity::columns::ONE_RM_PCT),
            Where::NotNull(entity::columns::EX_CAT),
            Where::NotNull(entity::columns::EX_ID),
        ];
        if let Some(keys) = exercises {
            if keys.is_empty() {
                return Ok(Vec::new());
            }
            conditions.push(Where::InMultiple(
                vec![entity::columns::EX_CAT, entity::columns::EX_ID],
                exercise_key_values(keys),
            ));
        }

        struct PercentStep {
            workout: u16,
            idx: u16,
            exercise: (u16, u16),
            weight: Option<f32>,
            pct: u8,
        }

        let steps: Vec<PercentStep> = WorkoutStepRepository::select()
            .columns(&[
                entity::columns::WORKOUT,
                entity::columns::IDX,
                entity::columns::EX_CAT,
                entity::columns::EX_ID,
                entity::columns::WEIGHT,
                entity::columns::ONE_RM_PCT,
            ])
            .where_(Where::And(conditions))
            .fetch_in(tx)
            .await?
            .iter()
            .map(|row| PercentStep {
                workout: row.get_u16(entity::columns::WORKOUT.as_ref()),
                idx: row.get_u16(entity::columns::IDX.as_ref()),
                exercise: (
                    row.get_u16(entity::columns::EX_CAT.as_ref()),
                    row.get_u16(entity::columns::EX_ID.as_ref()),
                ),
                weight: row.get_f32_nullable(entity::columns::WEIGHT.as_ref()),
                pct: row.get_u8(entity::columns::ONE_RM_PCT.as_ref()),
            })
            .collect();
        if steps.is_empty() {
            return Ok(Vec::new());
        }

        let keys: HashSet<(u16, u16)> = steps.iter().map(|s| s.exercise).collect();
        let exercises: HashMap<(u16, u16), Exercise> = Exercise::get_by_keys(tx, &keys)
            .await?
            .into_iter()
            .map(|e| ((e.category, e.id), e))
            .collect();
        let one_rms = current_one_rms(tx, Some(&keys), &exercises, now).await?;

        let mut changed: BTreeSet<u16> = BTreeSet::new();
        for PercentStep {
            workout,
            idx,
            exercise,
            weight,
            pct,
        } in steps
        {
            let increment = exercises.get(&exercise).map_or(0.0, |e| e.weight);
            let load = one_rms
                .get(&exercise)
                .map(|&one_rm| percent_load(one_rm, pct, increment))
                .filter(|&load| load > 0.0);
            if load == weight {
                continue;
            }
            WorkoutStepRepository::update()
                .set(entity::columns::WEIGHT, load.into())
                .where_(Where::And(vec![
                    Where::Eq(entity::columns::WORKOUT, workout.into()),
                    Where::Eq(entity::columns::IDX, idx.into()),
                ]))
                .execute_in(tx)
                .await?;
            changed.insert(workout);
        }

        if !changed.is_empty() {
            WorkoutDeviceRepository::update()
                .set(workout_device_cols::PENDING, true.into())
                .where_(Where::In(
                    workout_device_cols::WORKOUT,
                    changed.iter().map(|&w| w.into()).collect(),
                ))
                .execute_in(tx)
                .await?;
        }

        Ok(changed.into_iter().collect())
    }
}

/// Best estimated 1RM of the sets performed in the [`ONE_RM_WINDOW_WEEKS`] before `now`, of the
/// exercises in `keys` or of every exercise with `None`, by exercise. `exercises` tells which are
/// unilateral; those missing from it count as bilateral. Exercises without sets, or only unloaded
/// ones, are left out.
pub async fn current_one_rms(
    conn: &Connection,
    keys: Option<&HashSet<(u16, u16)>>,
    exercises: &HashMap<(u16, u16), Exercise>,
    now: u32,
) -> rusqlite_orm::errors::Result<HashMap<(u16, u16), f32>> {
    let since = now.saturating_sub(ONE_RM_WINDOW_WEEKS * 7 * 24 * 3600);
    let mut one_rms: HashMap<(u16, u16), f32> = HashMap::new();
    for (key, weight, reps) in SessionStep::get_sets_since(conn, keys, since).await? {
        let unilateral = exercises.get(&key).is_some_and(|e| e.unilateral);
        let estimate = estimate_1rm(weight, reps, unilateral);
        if estimate > 0.0 {
            let best = one_rms.entry(key).or_default();
            *best = best.max(estimate);
        }
    }
    Ok(one_rms)
}

/// Steps of workouts with progression enabled, only the enabled ones if `only_enabled`.
fn in_progression_workouts(only_enabled: bool) -> Where<WorkoutStep> {
    let mut conditions = vec![Where::Eq(workout_cols::PROGRESSION, true.into())];
    if only_enabled {
        conditions.push(Where::Eq(workout_cols::ENABLED, true.into()));
    }
    Where::InSub(
        entity::columns::WORKOUT,
        WorkoutRepository::select()
            .columns(&[workout_cols::ID])
            .where_(Where::And(conditions))
            .to_subquery(),
    )
}

/// Steps programmed below `weight` x `reps`; steps already at or above it are never lowered.
fn below_load(weight: f32, reps: u16) -> Where<WorkoutStep> {
    Where::Or(vec![
        weight_below(weight),
        Where::And(vec![
            weight_equals(weight),
            Where::Lt(entity::columns::REPS, reps.into()),
        ]),
    ])
}

/// Steps loaded below `weight`, a missing load counting as 0.
fn weight_below(weight: f32) -> Where<WorkoutStep> {
    let lt = Where::Lt(entity::columns::WEIGHT, weight.into());
    if weight > 0.0 {
        Where::Or(vec![Where::Null(entity::columns::WEIGHT), lt])
    } else {
        lt
    }
}

/// Steps loaded with exactly `weight`, a missing load counting as 0.
fn weight_equals(weight: f32) -> Where<WorkoutStep> {
    let eq = Where::Eq(entity::columns::WEIGHT, weight.into());
    if weight == 0.0 {
        Where::Or(vec![Where::Null(entity::columns::WEIGHT), eq])
    } else {
        eq
    }
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

impl StepType {
    /// Text the step type is stored as.
    pub fn as_str(&self) -> &'static str {
        match self {
            StepType::Exercise => "Exercise",
            StepType::Warmup => "Warmup",
            StepType::Recovery => "Recovery",
            StepType::Cooldown => "Cooldown",
            StepType::Rest => "Rest",
            StepType::Repeat => "Repeat",
            StepType::Other => "Other",
        }
    }
}

impl ToSql for StepType {
    fn to_sql(&self) -> rusqlite_orm::rusqlite::Result<ToSqlOutput<'_>> {
        Ok(ToSqlOutput::from(self.as_str()))
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
        Value::Text(value.as_str().to_string())
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
            _ => StepType::Other,
        }
    }
}

impl WorkoutStep {
    pub fn new(workout: u16, idx: u16, kind: StepType) -> Self {
        Self {
            workout,
            idx,
            kind,
            begin_idx: None,
            ex_cat: None,
            ex_id: None,
            reps: None,
            time: None,
            weight: None,
            one_rm_pct: None,
            notes: None,
            exercise: None,
        }
    }
    pub fn rest(workout: u16, idx: u16, time: Option<u32>, notes: Option<String>) -> Self {
        let mut inst = Self::new(workout, idx, StepType::Rest);
        inst.time = Some(time);
        inst.notes = notes;
        inst
    }

    pub fn repeat(workout: u16, idx: u16, from: u16, reps: u16) -> Self {
        let mut inst = Self::new(workout, idx, StepType::Repeat);
        inst.begin_idx = Some(from);
        inst.reps = Some(Some(reps));
        inst
    }

    #[allow(clippy::too_many_arguments)]
    pub fn active(
        workout: u16,
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

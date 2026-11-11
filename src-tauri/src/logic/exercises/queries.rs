use std::collections::{HashMap, HashSet};

use rusqlite_orm::{
    dao::Repository,
    database::DatabasePool,
    errors::DatabaseError,
    types::{order_by::OrderBy, where_clause::Where},
};

use crate::{
    dao::{
        body_metric::{self, BodyMetric, BodyMetricRepository},
        exercise::{self, Exercise, ExerciseRepository, GENERIC_EXERCISE_ID},
        exercise_category::ExerciseCategoryRepository,
        muscle_exercise::{self, MuscleExercise, MuscleExerciseRepository},
        session::{self, SessionRepository},
        session_step::{self, SessionStepRepository},
        workout::{self, WorkoutRepository},
        workout_device::{self, WorkoutDeviceRepository},
        workout_step::{self, StepType, WorkoutStep, WorkoutStepRepository},
    },
    dto::{
        exercises::{ExerciseDetails, ExerciseListItem, ProgressionSuggestion},
        sessions::SessionStepDto,
    },
};

/// Loads the personal record and full per-session set history for one exercise.
pub(super) fn fetch_exercise_details(
    conn: &rusqlite_orm::rusqlite::Connection,
    category: u16,
    id: u16,
    from_to: Option<(u32, u32)>,
) -> rusqlite_orm::errors::Result<ExerciseDetails> {
    exercise_details(conn, category, id, from_to, &mut None)
}

/// Same as [`fetch_exercise_details`], taking the body metrics (newest first) from `body_metrics`,
/// which is loaded on first need so several calls share a single query.
pub(crate) fn exercise_details(
    conn: &rusqlite_orm::rusqlite::Connection,
    category: u16,
    id: u16,
    from_to: Option<(u32, u32)>,
    body_metrics: &mut Option<Vec<BodyMetric>>,
) -> rusqlite_orm::errors::Result<ExerciseDetails> {
    let mut exercise = ExerciseRepository::select_by_id_in(conn, category, id)?.unwrap();
    exercise.fetch_muscles_relationship_in(conn)?;
    let mut res = ExerciseDetails::from(&exercise);

    let series = SessionStepRepository::select_by_exercise_in(
        conn,
        Some(category),
        Some(id),
        Some(&[
            OrderBy::Desc(session_step::entity::columns::SESSION),
            OrderBy::Asc(session_step::entity::columns::IDX),
        ]),
    )?;

    if !series.is_empty()
        && let Some(pr) = series.iter().find(|s| s.pr)
    {
        res.reps = pr.reps.unwrap_or_default();
        res.weight = pr.weight.unwrap_or_default();
        res.pr_date = pr.session;

        // The sessions with this exercise are exactly those of its sets
        let session_dates: HashSet<u32> = series.iter().map(|s| s.session).collect();
        let mut conditions = vec![Where::In(
            session::entity::columns::DATE,
            session_dates.into_iter().map(|d| d.into()).collect(),
        )];

        if let Some((from, to)) = from_to {
            conditions.push(Where::Gte(session::entity::columns::DATE, from.into()));
            conditions.push(Where::Lte(session::entity::columns::DATE, to.into()));
        }

        let workouts: HashMap<_, _> = SessionRepository::select()
            .columns(&[
                session::entity::columns::DATE,
                session::entity::columns::NAME,
            ])
            .where_(Where::And(conditions))
            .fetch_in(conn)?
            .iter()
            .map(|s| {
                (
                    s.get_u32(session::entity::columns::DATE.as_ref()),
                    s.get_text(session::entity::columns::NAME.as_ref()),
                )
            })
            .collect();

        let body_metrics = match body_metrics {
            Some(loaded) => loaded,
            None => body_metrics.insert(
                BodyMetricRepository::select()
                    .order_by(OrderBy::Desc(body_metric::entity::columns::DATE))
                    .fetch_in(conn)?,
            ),
        };

        let mut last_session = None;
        let mut ex_str = String::new();
        for serie in series {
            // Sessions outside the range aren't in `workouts`: their sets are skipped
            let Some(workout) = workouts.get(&serie.session) else {
                continue;
            };
            if last_session != Some(serie.session) {
                ex_str = format!("{}\n{}", workout, serie.session);
                res.workouts.push(ex_str.clone());
                last_session = Some(serie.session);
            }

            let entry = res.sessions.entry(ex_str.clone()).or_default();
            entry.weight = body_metrics.iter().find_map(|b| {
                if b.date < serie.session {
                    Some(b.weight)
                } else {
                    None
                }
            });
            entry.series.push(SessionStepDto::from(&serie));
        }
    }

    Ok(res)
}

/// Fills the muscles of every exercise in a single query, instead of one per exercise as the
/// relationship fetcher would.
pub(crate) fn fetch_exercises_muscles(
    conn: &rusqlite_orm::rusqlite::Connection,
    exercises: &mut [Exercise],
) -> rusqlite_orm::errors::Result<()> {
    if exercises.is_empty() {
        return Ok(());
    }

    let mut by_exercise: HashMap<(u16, u16), Vec<MuscleExercise>> = HashMap::new();
    for muscle in MuscleExerciseRepository::select()
        .where_(Where::InMultiple(
            vec![
                muscle_exercise::entity::columns::EX_CAT,
                muscle_exercise::entity::columns::EX_ID,
            ],
            exercises
                .iter()
                .map(|e| vec![e.category.into(), e.id.into()])
                .collect(),
        ))
        .fetch_in(conn)?
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

/// Exercises that can be picked, by category. The generic exercise of a category is left out when
/// the category already has a real exercise with its same name (e.g. `SQUAT` within squats).
pub(crate) fn exercises_catalog(
    conn: &rusqlite_orm::rusqlite::Connection,
) -> Result<HashMap<u16, Vec<u16>>, DatabaseError> {
    let category_names: HashMap<u16, String> = ExerciseCategoryRepository::select()
        .fetch_in(conn)?
        .into_iter()
        .map(|c| (c.id, c.name))
        .collect();
    let exercises = ExerciseRepository::select().fetch_in(conn)?;
    let named_as_category: HashSet<u16> = exercises
        .iter()
        .filter(|e| {
            e.id != GENERIC_EXERCISE_ID
                && category_names
                    .get(&e.category)
                    .is_some_and(|name| *name == e.name)
        })
        .map(|e| e.category)
        .collect();

    let mut catalog: HashMap<u16, Vec<u16>> = HashMap::new();
    for e in &exercises {
        if e.id != GENERIC_EXERCISE_ID || !named_as_category.contains(&e.category) {
            catalog.entry(e.category).or_default().push(e.id);
        }
    }

    Ok(catalog)
}

#[cfg(debug_assertions)]
pub(crate) fn update_all_prs(
    tx: &rusqlite_orm::rusqlite::Transaction,
) -> rusqlite_orm::errors::Result<()> {
    let mut exercises = HashSet::new();
    SessionStepRepository::select()
        .distinct(&[
            session_step::entity::columns::EX_CAT,
            session_step::entity::columns::EX_ID,
        ])
        .fetch_in(tx)?
        .iter()
        .filter_map(|r| {
            r.get_u16_nullable(session_step::entity::columns::EX_CAT.as_ref())
                .zip(r.get_u16_nullable(session_step::entity::columns::EX_ID.as_ref()))
        })
        .for_each(|e| {
            exercises.insert(e);
        });
    update_prs(tx, &exercises, &[]).map(|_| ())
}

/// Recomputes the `pr` flag for each affected exercise. Returns whether any of `sessions` holds a
/// new record.
pub(crate) fn update_prs(
    tx: &rusqlite_orm::rusqlite::Transaction,
    exercises: &HashSet<(u16, u16)>,
    sessions: &[u32],
) -> rusqlite_orm::errors::Result<bool> {
    let mut new_prs = false;

    let sessions = sessions.to_vec();

    let mut update_false_conditions = vec![];
    let mut update_true_conditions = vec![];

    for exer in exercises {
        update_false_conditions.push(vec![exer.0.into(), exer.1.into()]);
        if let Some(pr) = SessionStepRepository::select()
            .where_(Where::And(vec![
                Where::Eq(session_step::entity::columns::EX_CAT, exer.0.into()),
                Where::Eq(session_step::entity::columns::EX_ID, exer.1.into()),
            ]))
            .order_by(OrderBy::Desc(session_step::entity::columns::WEIGHT))
            .order_by(OrderBy::Desc(session_step::entity::columns::REPS))
            .order_by(OrderBy::Asc(session_step::entity::columns::SESSION))
            .order_by(OrderBy::Asc(session_step::entity::columns::IDX))
            .limit(1)
            .fetch_one_in(tx)?
        {
            update_true_conditions.push(vec![pr.session.into(), pr.idx.into()]);
            new_prs = new_prs || sessions.contains(&pr.session);
        }
    }

    if !update_true_conditions.is_empty() {
        SessionStepRepository::update()
            .set(session_step::entity::columns::PR, false.into())
            .where_(Where::And(vec![
                Where::InMultiple(
                    vec![
                        session_step::entity::columns::EX_CAT,
                        session_step::entity::columns::EX_ID,
                    ],
                    update_false_conditions,
                ),
                Where::Eq(session_step::entity::columns::PR, true.into()),
            ]))
            .execute_in(tx)?;
        SessionStepRepository::update()
            .set(session_step::entity::columns::PR, true.into())
            .where_(Where::InMultiple(
                vec![
                    session_step::entity::columns::SESSION,
                    session_step::entity::columns::IDX,
                ],
                update_true_conditions,
            ))
            .execute_in(tx)?;
    }

    Ok(new_prs)
}

pub(crate) fn fetch_progression_suggestion(
    conn: &rusqlite_orm::rusqlite::Connection,
    category: u16,
    id: u16,
) -> rusqlite_orm::errors::Result<Option<ProgressionSuggestion>> {
    match ExerciseRepository::select_by_id_in(conn, category, id)? {
        Some(exercise) => progression_suggestion_for(conn, &exercise),
        None => Ok(None),
    }
}

/// Exercises with progression enabled that currently have a suggestion.
pub(crate) fn progression_suggestions(
    conn: &rusqlite_orm::rusqlite::Connection,
) -> rusqlite_orm::errors::Result<HashSet<(u16, u16)>> {
    let candidates = ExerciseRepository::select()
        .where_(progression_candidates())
        .fetch_in(conn)?;
    Ok(progression_suggestions_for(conn, &candidates)?
        .into_keys()
        .collect())
}

/// Exercises with progression enabled that could have a suggestion: performed at least once and programmed in an enabled workout.
/// Prunes the catalog in SQL so the per-exercise checks only run for the handful of exercises actually in use.
pub(super) fn progression_candidates() -> Where<Exercise> {
    let performed = SessionStepRepository::select()
        .distinct(&[
            session_step::entity::columns::EX_CAT,
            session_step::entity::columns::EX_ID,
        ])
        .where_(Where::Eq(
            session_step::entity::columns::KIND,
            StepType::Exercise.into(),
        ))
        .to_subquery();
    let programmed = WorkoutStepRepository::select()
        .distinct(&[
            workout_step::entity::columns::EX_CAT,
            workout_step::entity::columns::EX_ID,
        ])
        .where_(Where::And(vec![
            exercise_step(),
            Where::InSub(
                workout_step::entity::columns::WORKOUT,
                WorkoutRepository::select()
                    .columns(&[workout::entity::columns::NAME])
                    .where_(Where::Eq(workout::entity::columns::ENABLED, true.into()))
                    .to_subquery(),
            ),
        ]))
        .to_subquery();
    let key = || {
        vec![
            exercise::entity::columns::CATEGORY,
            exercise::entity::columns::ID,
        ]
    };
    Where::And(vec![
        Where::Eq(exercise::entity::columns::PROGRESSION, true.into()),
        Where::InMultipleSub(key(), performed),
        Where::InMultipleSub(key(), programmed),
    ])
}

/// A performed set, as weight (0 if unloaded) and reps.
type Set = (f32, u16);

/// Suggestion for an already loaded `exercise`, `None` if it has progression disabled.
pub(super) fn progression_suggestion_for(
    conn: &rusqlite_orm::rusqlite::Connection,
    exercise: &Exercise,
) -> rusqlite_orm::errors::Result<Option<ProgressionSuggestion>> {
    Ok(progression_suggestions_for(conn, [exercise])?.remove(&(exercise.category, exercise.id)))
}

/// Pending suggestions for the already loaded `exercises`, by key. Exercises with progression
/// disabled or nothing to suggest are left out. Takes two queries whatever the number of exercises.
pub(super) fn progression_suggestions_for<'a>(
    conn: &rusqlite_orm::rusqlite::Connection,
    exercises: impl IntoIterator<Item = &'a Exercise>,
) -> rusqlite_orm::errors::Result<HashMap<(u16, u16), ProgressionSuggestion>> {
    let exercises: HashMap<(u16, u16), &Exercise> = exercises
        .into_iter()
        .filter(|e| e.progression)
        .map(|e| ((e.category, e.id), e))
        .collect();
    if exercises.is_empty() {
        return Ok(HashMap::new());
    }
    let key_values = |keys: &mut dyn Iterator<Item = &(u16, u16)>| {
        keys.map(|k| vec![k.0.into(), k.1.into()]).collect()
    };

    // Sets of the latest session of each exercise
    let mut latest: HashMap<(u16, u16), (u32, Vec<Set>)> = HashMap::new();
    for row in SessionStepRepository::select()
        .columns(&[
            session_step::entity::columns::EX_CAT,
            session_step::entity::columns::EX_ID,
            session_step::entity::columns::SESSION,
            session_step::entity::columns::WEIGHT,
            session_step::entity::columns::REPS,
        ])
        .where_(Where::And(vec![
            Where::Eq(
                session_step::entity::columns::KIND,
                StepType::Exercise.into(),
            ),
            Where::InMultiple(
                vec![
                    session_step::entity::columns::EX_CAT,
                    session_step::entity::columns::EX_ID,
                ],
                key_values(&mut exercises.keys()),
            ),
        ]))
        .fetch_in(conn)?
    {
        let key = (
            row.get_u16(session_step::entity::columns::EX_CAT.as_ref()),
            row.get_u16(session_step::entity::columns::EX_ID.as_ref()),
        );
        let session = row.get_u32(session_step::entity::columns::SESSION.as_ref());
        let set = (
            row.get_f32_nullable(session_step::entity::columns::WEIGHT.as_ref())
                .unwrap_or_default(),
            row.get_u16_nullable(session_step::entity::columns::REPS.as_ref())
                .unwrap_or_default(),
        );
        let entry = latest.entry(key).or_insert((session, Vec::new()));
        if session > entry.0 {
            *entry = (session, Vec::new());
        }
        if session == entry.0 {
            entry.1.push(set);
        }
    }

    let candidates: HashMap<(u16, u16), ProgressionSuggestion> = latest
        .iter()
        .filter_map(|(key, (_, sets))| {
            Some((*key, suggestion_from_sets(exercises.get(key)?, sets)?))
        })
        .collect();
    if candidates.is_empty() {
        return Ok(HashMap::new());
    }

    // Steps of disabled workouts don't count: they alone would suggest what is already programmed
    let mut pending = HashSet::new();
    for row in WorkoutStepRepository::select()
        .columns(&[
            workout_step::entity::columns::EX_CAT,
            workout_step::entity::columns::EX_ID,
            workout_step::entity::columns::WEIGHT,
            workout_step::entity::columns::REPS,
        ])
        .where_(Where::And(vec![
            exercise_step(),
            Where::NotNull(workout_step::entity::columns::REPS),
            Where::InMultiple(
                vec![
                    workout_step::entity::columns::EX_CAT,
                    workout_step::entity::columns::EX_ID,
                ],
                key_values(&mut candidates.keys()),
            ),
            Where::InSub(
                workout_step::entity::columns::WORKOUT,
                WorkoutRepository::select()
                    .columns(&[workout::entity::columns::NAME])
                    .where_(Where::Eq(workout::entity::columns::ENABLED, true.into()))
                    .to_subquery(),
            ),
        ]))
        .fetch_in(conn)?
    {
        let key = (
            row.get_u16(workout_step::entity::columns::EX_CAT.as_ref()),
            row.get_u16(workout_step::entity::columns::EX_ID.as_ref()),
        );
        let Some(suggestion) = candidates.get(&key) else {
            continue;
        };
        // Same as `below_suggestion`, a missing load counting as 0
        let weight = row
            .get_f32_nullable(workout_step::entity::columns::WEIGHT.as_ref())
            .unwrap_or_default();
        let reps = row.get_u16(workout_step::entity::columns::REPS.as_ref());
        if weight < suggestion.weight || (weight == suggestion.weight && reps < suggestion.reps) {
            pending.insert(key);
        }
    }

    Ok(candidates
        .into_iter()
        .filter(|(key, _)| pending.contains(key))
        .collect())
}

/// Load to suggest for `exercise` given the `sets` of its latest session, `None` if they don't
/// share a weight or don't all reach its rep range.
fn suggestion_from_sets(exercise: &Exercise, sets: &[Set]) -> Option<ProgressionSuggestion> {
    let &(weight, _) = sets.first()?;
    if sets.iter().any(|&(w, _)| w != weight) {
        return None;
    }

    let range: Vec<_> = exercise
        .reps
        .split('-')
        .filter_map(|s| s.parse::<u16>().ok())
        .collect();
    if range.is_empty() {
        return None;
    }

    // Index of the highest range step reached by each set; every set must reach the range and the weakest one rules
    let bracket = sets
        .iter()
        .map(|&(_, reps)| {
            let reps = if exercise.unilateral { reps / 2 } else { reps };
            range.iter().rposition(|&r| reps >= r)
        })
        .collect::<Option<Vec<_>>>()?
        .into_iter()
        .min()?;

    let (weight, reps) = match bracket {
        i if i == range.len() - 1 => (weight + exercise.weight, range[0]),
        i => (weight, range[i + 1]),
    };
    Some(ProgressionSuggestion {
        weight,
        reps: if exercise.unilateral { reps * 2 } else { reps },
    })
}

/// Only exercise steps progress: warm-ups and the like keep their own load.
pub(super) fn exercise_step() -> Where<WorkoutStep> {
    Where::Eq(
        workout_step::entity::columns::KIND,
        StepType::Exercise.into(),
    )
}

/// Steps programmed below `suggestion`; steps already at or above it are never lowered.
pub(super) fn below_suggestion(suggestion: &ProgressionSuggestion) -> Where<WorkoutStep> {
    Where::Or(vec![
        weight_below(suggestion.weight),
        Where::And(vec![
            weight_equals(suggestion.weight),
            Where::Lt(workout_step::entity::columns::REPS, suggestion.reps.into()),
        ]),
    ])
}

/// Steps loaded below `weight`, a missing load counting as 0.
pub(super) fn weight_below(weight: f32) -> Where<WorkoutStep> {
    let lt = Where::Lt(workout_step::entity::columns::WEIGHT, weight.into());
    if weight > 0.0 {
        Where::Or(vec![Where::Null(workout_step::entity::columns::WEIGHT), lt])
    } else {
        lt
    }
}

/// Steps loaded with exactly `weight`, a missing load counting as 0.
pub(super) fn weight_equals(weight: f32) -> Where<WorkoutStep> {
    let eq = Where::Eq(workout_step::entity::columns::WEIGHT, weight.into());
    if weight == 0.0 {
        Where::Or(vec![Where::Null(workout_step::entity::columns::WEIGHT), eq])
    } else {
        eq
    }
}

/// Stores the exercise notes and queues for upload the workouts inheriting them.
/// Returns every workout inheriting the notes.
pub(crate) fn update_exercise_notes(
    tx: &rusqlite_orm::rusqlite::Transaction,
    category: u16,
    id: u16,
    notes: Option<String>,
) -> rusqlite_orm::errors::Result<Vec<String>> {
    ExerciseRepository::update()
        .set(exercise::entity::columns::NOTES, notes.into())
        .where_(Where::And(vec![
            Where::Eq(exercise::entity::columns::CATEGORY, category.into()),
            Where::Eq(exercise::entity::columns::ID, id.into()),
        ]))
        .execute_in(tx)?;

    let workouts: Vec<String> = WorkoutStepRepository::select()
        .distinct(&[workout_step::entity::columns::WORKOUT])
        .where_(Where::And(vec![
            Where::Eq(workout_step::entity::columns::EX_CAT, category.into()),
            Where::Eq(workout_step::entity::columns::EX_ID, id.into()),
            Where::Or(vec![
                Where::Null(workout_step::entity::columns::NOTES),
                Where::Eq(workout_step::entity::columns::NOTES, "".into()),
            ]),
        ]))
        .fetch_in(tx)?
        .iter()
        .map(|r| r.get_text(workout_step::entity::columns::WORKOUT.as_ref()))
        .collect();

    if !workouts.is_empty() {
        // On a disabled workout `pending` means deletion from the device; enabling it queues the upload
        WorkoutDeviceRepository::update()
            .set(workout_device::entity::columns::PENDING, true.into())
            .where_(Where::And(vec![
                Where::In(
                    workout_device::entity::columns::WORKOUT,
                    workouts.iter().map(|w| w.clone().into()).collect(),
                ),
                Where::InSub(
                    workout_device::entity::columns::WORKOUT,
                    WorkoutRepository::select()
                        .distinct(&[workout::entity::columns::NAME])
                        .where_(Where::Eq(workout::entity::columns::ENABLED, true.into()))
                        .to_subquery(),
                ),
            ]))
            .execute_in(tx)?;
    }

    Ok(workouts)
}

/// Whether any exercise with progression enabled currently has a suggestion pending.
pub(super) fn any_progression_suggestion(
    conn: &rusqlite_orm::rusqlite::Connection,
) -> rusqlite_orm::errors::Result<bool> {
    progression_suggestions(conn).map(|suggested| !suggested.is_empty())
}

/// The exercises with a personal record, annotated with it, plus those planned in some workout
/// step that have none yet.
pub(super) fn exercise_list(
    conn: &rusqlite_orm::rusqlite::Connection,
) -> rusqlite_orm::errors::Result<Vec<ExerciseListItem>> {
    // Only exercise sets can hold a record; the filter just keeps the unwraps below safe
    let prs: Vec<_> = SessionStepRepository::select_by_personal_records_in(conn, true, None)?
        .into_iter()
        .filter(|s| s.ex_cat.is_some() && s.ex_id.is_some())
        .collect();
    let pr_keys: HashSet<(u16, u16)> = prs
        .iter()
        .filter_map(|s| Some((s.ex_cat?, s.ex_id?)))
        .collect();

    let mut planned: Vec<(u16, u16)> = WorkoutStepRepository::select()
        .where_(Where::And(vec![
            Where::NotNull(workout_step::entity::columns::EX_CAT),
            Where::NotNull(workout_step::entity::columns::EX_ID),
        ]))
        .fetch_in(conn)?
        .iter()
        .filter_map(|s| Some((s.ex_cat?, s.ex_id?)))
        .filter(|key| !pr_keys.contains(key))
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();
    planned.sort();

    let keys: Vec<_> = pr_keys.iter().chain(planned.iter()).collect();
    let exercises: HashMap<(u16, u16), Exercise> = if keys.is_empty() {
        HashMap::new()
    } else {
        ExerciseRepository::select()
            .where_(Where::InMultiple(
                vec![
                    exercise::entity::columns::CATEGORY,
                    exercise::entity::columns::ID,
                ],
                keys.iter().map(|k| vec![k.0.into(), k.1.into()]).collect(),
            ))
            .fetch_in(conn)?
            .into_iter()
            .map(|e| ((e.category, e.id), e))
            .collect()
    };

    let suggested_progressions = progression_suggestions_for(
        conn,
        exercises
            .iter()
            .filter(|(key, _)| pr_keys.contains(key))
            .map(|(_, e)| e),
    )?;

    let mut items: Vec<_> = prs
        .iter()
        .map(|pr| {
            let key = (pr.ex_cat.unwrap(), pr.ex_id.unwrap());
            let exercise = exercises.get(&key);
            ExerciseListItem {
                category: key.0,
                id: key.1,
                reps: pr.reps.unwrap_or_default(),
                weight: pr.weight.unwrap_or_default(),
                date: pr.session,
                unilateral: exercise.is_some_and(|e| e.unilateral),
                progression: suggested_progressions.contains_key(&key),
                notes: exercise.and_then(|e| e.notes.clone()),
                used: true,
            }
        })
        .collect();

    // Planned exercises may not have been recorded yet, so they can lack a catalog row
    items.extend(planned.iter().map(|key| match exercises.get(key) {
        Some(exercise) => ExerciseListItem::from(exercise),
        None => ExerciseListItem {
            category: key.0,
            id: key.1,
            reps: 0,
            weight: 0.0,
            date: 0,
            unilateral: false,
            progression: false,
            notes: None,
            used: false,
        },
    }));

    Ok(items)
}

/// Marks the exercise as performed one side at a time, or not.
pub(super) async fn set_unilateral(
    database: &DatabasePool,
    category: u16,
    id: u16,
    value: bool,
) -> rusqlite_orm::errors::Result<()> {
    ExerciseRepository::update()
        .set(exercise::entity::columns::UNILATERAL, value.into())
        .where_(by_key(category, id))
        .execute(database)
        .await?;
    Ok(())
}

/// Enables or disables progression suggestions for the exercise.
pub(super) async fn set_progression(
    database: &DatabasePool,
    category: u16,
    id: u16,
    value: bool,
) -> rusqlite_orm::errors::Result<()> {
    ExerciseRepository::update()
        .set(exercise::entity::columns::PROGRESSION, value.into())
        .where_(by_key(category, id))
        .execute(database)
        .await?;
    Ok(())
}

/// Stores the rep range (e.g. `8-10-12`) and weight increment used to suggest progressions.
pub(super) async fn set_progression_parameters(
    database: &DatabasePool,
    category: u16,
    id: u16,
    reps: String,
    weight: f32,
) -> rusqlite_orm::errors::Result<()> {
    ExerciseRepository::update()
        .set(exercise::entity::columns::REPS, reps.into())
        .set(exercise::entity::columns::WEIGHT, weight.into())
        .where_(by_key(category, id))
        .execute(database)
        .await?;
    Ok(())
}

fn by_key(category: u16, id: u16) -> Where<Exercise> {
    Where::And(vec![
        Where::Eq(exercise::entity::columns::CATEGORY, category.into()),
        Where::Eq(exercise::entity::columns::ID, id.into()),
    ])
}

/// Highest load programmed for the exercise across workouts, as reps and weight.
pub(super) async fn progression_status(
    database: &DatabasePool,
    category: u16,
    id: u16,
) -> rusqlite_orm::errors::Result<Option<ProgressionSuggestion>> {
    Ok(WorkoutStepRepository::select()
        .columns(&[
            workout_step::entity::columns::REPS,
            workout_step::entity::columns::WEIGHT,
        ])
        .where_(Where::And(vec![
            Where::Eq(workout_step::entity::columns::EX_CAT, category.into()),
            Where::Eq(workout_step::entity::columns::EX_ID, id.into()),
            exercise_step(),
        ]))
        .order_by(OrderBy::Desc(workout_step::entity::columns::WEIGHT))
        .order_by(OrderBy::Desc(workout_step::entity::columns::REPS))
        .fetch_one(database)
        .await?
        .map(|r| ProgressionSuggestion {
            reps: r
                .get_u16_nullable(workout_step::entity::columns::REPS.as_ref())
                .unwrap_or_default(),
            weight: r
                .get_f32_nullable(workout_step::entity::columns::WEIGHT.as_ref())
                .unwrap_or_default(),
        }))
}

/// Raises the exercise steps programmed below `suggestion` and queues their workouts for the
/// devices. Returns whether any step changed and the workouts programming the exercise.
pub(super) fn apply_progression(
    tx: &rusqlite_orm::rusqlite::Transaction,
    category: u16,
    id: u16,
    suggestion: &ProgressionSuggestion,
) -> rusqlite_orm::errors::Result<(bool, Vec<String>)> {
    // Reps go first: the weight update would otherwise change which steps are below
    let mut modified = WorkoutStepRepository::update()
        .set(workout_step::entity::columns::REPS, suggestion.reps.into())
        .where_(Where::And(vec![
            Where::Eq(workout_step::entity::columns::EX_CAT, category.into()),
            Where::Eq(workout_step::entity::columns::EX_ID, id.into()),
            exercise_step(),
            Where::NotNull(workout_step::entity::columns::REPS),
            below_suggestion(suggestion),
        ]))
        .execute_in(tx)?;
    modified += WorkoutStepRepository::update()
        .set(
            workout_step::entity::columns::WEIGHT,
            suggestion.weight.into(),
        )
        .where_(Where::And(vec![
            Where::Eq(workout_step::entity::columns::EX_CAT, category.into()),
            Where::Eq(workout_step::entity::columns::EX_ID, id.into()),
            exercise_step(),
            weight_below(suggestion.weight),
        ]))
        .execute_in(tx)?;

    let mut workouts = Vec::new();
    if modified > 0 {
        workouts = WorkoutStepRepository::select()
            .distinct(&[workout_step::entity::columns::WORKOUT])
            .where_(Where::And(vec![
                Where::Eq(workout_step::entity::columns::EX_CAT, category.into()),
                Where::Eq(workout_step::entity::columns::EX_ID, id.into()),
                exercise_step(),
            ]))
            .fetch_in(tx)?
            .iter()
            .map(|r| r.get_text(workout_step::entity::columns::WORKOUT.as_ref()))
            .collect();
        WorkoutDeviceRepository::update()
            .set(workout_device::entity::columns::PENDING, true.into())
            .where_(Where::InSub(
                workout_device::entity::columns::WORKOUT,
                WorkoutStepRepository::select()
                    .distinct(&[workout_step::entity::columns::WORKOUT])
                    .where_(Where::And(vec![
                        Where::Eq(workout_step::entity::columns::EX_CAT, category.into()),
                        Where::Eq(workout_step::entity::columns::EX_ID, id.into()),
                        exercise_step(),
                    ]))
                    .to_subquery(),
            ))
            .execute_in(tx)?;
    }

    Ok((modified > 0, workouts))
}

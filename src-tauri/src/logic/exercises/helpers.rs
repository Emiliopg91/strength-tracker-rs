use std::collections::{HashMap, HashSet};

use rusqlite_orm::rusqlite::Connection;

use crate::{
    dao::{
        body_metric::BodyMetric,
        exercise::{Exercise, GENERIC_EXERCISE_ID},
        exercise_category::ExerciseCategory,
        session::Session,
        session_step::SessionStep,
        workout_step::WorkoutStep,
    },
    dto::{
        exercises::{ExerciseDetails, ExerciseListItem, ProgressionSuggestion},
        sessions::SessionStepDto,
    },
};

/// Loads the personal record and full per-session set history for one exercise.
pub(super) async fn fetch_exercise_details(
    conn: &Connection,
    category: u16,
    id: u16,
    from_to: Option<(u32, u32)>,
) -> rusqlite_orm::errors::Result<ExerciseDetails> {
    exercise_details(conn, category, id, from_to, &mut None).await
}

/// Same as [`fetch_exercise_details`], taking the body metrics (newest first) from `body_metrics`,
/// which is loaded on first need so several calls share a single query.
pub(crate) async fn exercise_details(
    conn: &Connection,
    category: u16,
    id: u16,
    from_to: Option<(u32, u32)>,
    body_metrics: &mut Option<Vec<BodyMetric>>,
) -> rusqlite_orm::errors::Result<ExerciseDetails> {
    let exercise = Exercise::get_with_muscles(conn, category, id).await?;
    let mut res = ExerciseDetails::from(&exercise);

    let series = SessionStep::get_by_exercise(conn, category, id).await?;

    if !series.is_empty()
        && let Some(pr) = series.iter().find(|s| s.pr)
    {
        res.reps = pr.reps.unwrap_or_default();
        res.weight = pr.weight.unwrap_or_default();
        res.pr_date = pr.session;

        // The sessions with this exercise are exactly those of its sets
        let session_dates: HashSet<u32> = series.iter().map(|s| s.session).collect();
        let workouts = Session::get_names_by_dates(conn, &session_dates, from_to).await?;

        let body_metrics = match body_metrics {
            Some(loaded) => loaded,
            None => body_metrics.insert(BodyMetric::get_by_range(conn, None).await?),
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

/// Exercises that can be picked, by category. The generic exercise of a category is left out when
/// the category already has a real exercise with its same name (e.g. `SQUAT` within squats).
pub(crate) async fn exercises_catalog(
    conn: &Connection,
) -> rusqlite_orm::errors::Result<HashMap<u16, Vec<u16>>> {
    let category_names: HashMap<u16, String> = ExerciseCategory::get_all(conn)
        .await?
        .into_iter()
        .map(|c| (c.id, c.name))
        .collect();
    let exercises = Exercise::get_all(conn).await?;
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

/// Recomputes the personal records of every exercise with some recorded set.
#[cfg(debug_assertions)]
pub(crate) async fn update_all_prs(
    tx: &rusqlite_orm::rusqlite::Transaction<'_>,
) -> rusqlite_orm::errors::Result<()> {
    let exercises = SessionStep::get_performed_exercises(tx).await?;
    SessionStep::update_prs(tx, &exercises, &[])
        .await
        .map(|_| ())
}

/// Pending suggestion for the exercise, `None` if it doesn't exist, has progression disabled or
/// has nothing to suggest.
pub(crate) async fn fetch_progression_suggestion(
    conn: &Connection,
    category: u16,
    id: u16,
) -> rusqlite_orm::errors::Result<Option<ProgressionSuggestion>> {
    let Some(exercise) = Exercise::get(conn, category, id).await? else {
        return Ok(None);
    };
    Ok(progression_suggestions_for(conn, [&exercise])
        .await?
        .remove(&(exercise.category, exercise.id)))
}

/// Exercises with progression enabled that currently have a suggestion.
pub(crate) async fn progression_suggestions(
    conn: &Connection,
) -> rusqlite_orm::errors::Result<HashSet<(u16, u16)>> {
    let candidates = Exercise::get_progression_candidates(conn).await?;
    Ok(progression_suggestions_for(conn, &candidates)
        .await?
        .into_keys()
        .collect())
}

/// Which of the exercises in `keys` currently have a suggestion, in two queries plus one to load
/// them, whatever their number.
pub(crate) async fn progression_suggestions_among(
    conn: &Connection,
    keys: &HashSet<(u16, u16)>,
) -> rusqlite_orm::errors::Result<HashSet<(u16, u16)>> {
    let exercises = Exercise::get_by_keys(conn, keys).await?;
    Ok(progression_suggestions_for(conn, &exercises)
        .await?
        .into_keys()
        .collect())
}

/// Whether any exercise with progression enabled currently has a suggestion pending.
pub(super) async fn any_progression_suggestion(
    conn: &Connection,
) -> rusqlite_orm::errors::Result<bool> {
    progression_suggestions(conn)
        .await
        .map(|suggested| !suggested.is_empty())
}

/// A performed set, as weight (0 if unloaded) and reps.
type Set = (f32, u16);

/// Pending suggestions for the already loaded `exercises`, by key. Exercises with progression
/// disabled or nothing to suggest are left out. Takes two queries whatever the number of exercises.
async fn progression_suggestions_for<'a>(
    conn: &Connection,
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

    // Sets of the latest session of each exercise
    let keys: HashSet<(u16, u16)> = exercises.keys().copied().collect();
    let mut latest: HashMap<(u16, u16), Vec<Set>> = HashMap::new();
    for (key, _, weight, reps) in SessionStep::get_latest_exercise_sets(conn, &keys).await? {
        latest.entry(key).or_default().push((weight, reps));
    }

    let candidates: HashMap<(u16, u16), ProgressionSuggestion> = latest
        .iter()
        .filter_map(|(key, sets)| Some((*key, suggestion_from_sets(exercises.get(key)?, sets)?)))
        .collect();
    if candidates.is_empty() {
        return Ok(HashMap::new());
    }

    // Steps of disabled workouts don't count: they alone would suggest what is already programmed
    let keys: HashSet<(u16, u16)> = candidates.keys().copied().collect();
    let mut pending = HashSet::new();
    for (key, weight, reps) in WorkoutStep::get_programmed_loads(conn, &keys).await? {
        let Some(suggestion) = candidates.get(&key) else {
            continue;
        };
        // Steps programmed below the suggestion, those already at or above it never lowered
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

/// The exercises with a personal record, annotated with it, plus those planned in some workout
/// step that have none yet.
pub(super) async fn exercise_list(
    conn: &Connection,
) -> rusqlite_orm::errors::Result<Vec<ExerciseListItem>> {
    // Only exercise sets can hold a record; the filter just keeps the unwraps below safe
    let prs: Vec<_> = SessionStep::get_personal_records(conn)
        .await?
        .into_iter()
        .filter(|s| s.ex_cat.is_some() && s.ex_id.is_some())
        .collect();
    let pr_keys: HashSet<(u16, u16)> = prs
        .iter()
        .filter_map(|s| Some((s.ex_cat?, s.ex_id?)))
        .collect();

    let mut planned: Vec<(u16, u16)> = WorkoutStep::get_planned_exercises(conn)
        .await?
        .into_iter()
        .filter(|key| !pr_keys.contains(key))
        .collect();
    planned.sort();

    let exercises: HashMap<(u16, u16), Exercise> =
        Exercise::get_by_keys(conn, pr_keys.iter().chain(planned.iter()))
            .await?
            .into_iter()
            .map(|e| ((e.category, e.id), e))
            .collect();

    let suggested_progressions = progression_suggestions_for(
        conn,
        exercises
            .iter()
            .filter(|(key, _)| pr_keys.contains(key))
            .map(|(_, e)| e),
    )
    .await?;

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
    items.extend(planned.iter().map(|key| {
        exercises.get(key).map_or_else(
            || ExerciseListItem {
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
            ExerciseListItem::from,
        )
    }));

    Ok(items)
}

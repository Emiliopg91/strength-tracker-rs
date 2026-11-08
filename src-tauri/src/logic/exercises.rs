use std::collections::{HashMap, HashSet};

use rusqlite_orm::{
    dao::Repository,
    errors::DatabaseError,
    types::{order_by::OrderBy, value::Value, where_clause::Where},
};
use tauri::{AppHandle, Runtime};
use tauri_plugin_log::log::info;

use crate::{
    ConnectLock, SettingsLock,
    dao::{
        body_metric::{self, BodyMetricRepository},
        exercise::{self, ExerciseRepository},
        session::{self, SessionRepository},
        session_step::{self, SessionStepRepository},
        workout::{self, WorkoutRepository},
        workout_device::{self, WorkoutDeviceRepository},
        workout_step::{self, StepType, WorkoutStep, WorkoutStepRepository},
    },
    dto::{
        exercises::{ExerciseDetails, ExerciseListItem, ProgressionSuggestion},
        notifications::{NotificationDefinition, NotificationKind},
        sessions::SessionStepDto,
    },
    logic::{
        connect::send_workout_connect, devices::push_pending_to_usb,
        notifications::show_notification, report_error, row_f64_nullable, row_int,
        row_int_nullable, row_text, run_blocking,
    },
    utils::translations::{Languages, translate},
};

/// Returns every exercise in the catalog, each annotated with its current personal record.
pub async fn get_exercises<R: Runtime>(
    app: &AppHandle<R>,
    settings: &SettingsLock,
) -> Result<Vec<ExerciseListItem>, String> {
    info!("Getting exercises list...");
    let res = run_blocking(app, move |database| {
        database.run_in_connection(|conn| {
            let prs = SessionStepRepository::select()
                .where_(Where::And(vec![
                    Where::Eq(session_step::entity::columns::PR, true.into()),
                    Where::NotEq(session_step::entity::columns::KIND, StepType::Rest.into()),
                ]))
                .fetch_in(conn)?;
            let exercises = ExerciseRepository::select()
                .where_(Where::And(vec![Where::InMultiple(
                    vec![
                        exercise::entity::columns::CATEGORY,
                        exercise::entity::columns::ID,
                    ],
                    prs.iter()
                        .map(|e| vec![e.ex_cat.into(), e.ex_id.into()])
                        .collect(),
                )]))
                .fetch_in(conn)?;
            let mut suggested_progressions = Vec::new();
            for exercise in &exercises {
                if fetch_progression_suggestion(conn, exercise.category, exercise.id)?.is_some() {
                    suggested_progressions.push((exercise.category, exercise.id));
                }
            }

            Ok(prs
                .iter()
                .map(|pr| ExerciseListItem {
                    category: pr.ex_cat.unwrap(),
                    id: pr.ex_id.unwrap(),
                    // Rest steps, the only ones without these, can't be personal records
                    reps: pr.reps.unwrap_or_default(),
                    weight: pr.weight.unwrap_or_default(),
                    date: pr.session,
                    unilateral: exercises
                        .iter()
                        .find(|e| e.category == pr.ex_cat.unwrap() && e.id == pr.ex_id.unwrap())
                        .unwrap()
                        .unilateral,
                    progression: suggested_progressions
                        .contains(&(pr.ex_cat.unwrap(), pr.ex_id.unwrap())),
                    notes: exercises
                        .iter()
                        .find(|e| e.category == pr.ex_cat.unwrap() && e.id == pr.ex_id.unwrap())
                        .unwrap()
                        .notes
                        .clone(),
                })
                .collect::<Vec<_>>())
        })
    })
    .await;

    match res {
        Ok(l) => {
            info!("Retreived {} exercises", l.len());
            Ok(l)
        }
        Err(e) => Err(report_error(
            e,
            settings.read().unwrap().language,
            "error_exercise_list",
            "Error getting exercises list",
        )),
    }
}

/// Returns the personal record and full per-session set history for one exercise.
pub async fn get_exercise_details<R: Runtime>(
    app: &AppHandle<R>,
    settings: &SettingsLock,
    category: u16,
    id: u16,
) -> Result<ExerciseDetails, String> {
    info!(
        "Getting details for exercise with category {} and id {}...",
        category, id
    );
    let res = run_blocking(app, move |database| {
        database.run_in_connection(|conn| Ok(fetch_exercise_details(conn, category, id, None)?))
    })
    .await;
    match res {
        Ok(l) => {
            info!("Found details for exercise {} - {}", category, id);
            Ok(l)
        }
        Err(e) => Err(report_error(
            e,
            settings.read().unwrap().language,
            "error_exercise_details",
            "Error getting exercise details",
        )),
    }
}

/// Loads the personal record and full per-session set history for one exercise.
pub fn fetch_exercise_details(
    conn: &rusqlite_orm::rusqlite::Connection,
    category: u16,
    id: u16,
    from_to: Option<(u32, u32)>,
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

    if !series.is_empty() {
        let mut session_dates = HashSet::new();
        for serie in &series {
            session_dates.insert(serie.session);
        }

        let chart_session = SessionRepository::select()
            .where_(Where::And(vec![
                Where::In(
                    session::entity::columns::DATE,
                    session_dates.into_iter().map(|d| d.into()).collect(),
                ),
                Where::Eq(session::entity::columns::SHOW_EXERCISES_CHART, true.into()),
            ]))
            .fetch_in(conn)?
            .iter()
            .map(|s| s.date)
            .collect();

        let pr = series.iter().find(|s| s.pr).unwrap();
        res.reps = pr.reps.unwrap_or_default();
        res.weight = pr.weight.unwrap_or_default();
        res.pr_date = pr.session;

        let mut conditions = Vec::new();
        conditions.push(Where::InSub(
            session::entity::columns::DATE,
            SessionStepRepository::select()
                .distinct(&[session_step::entity::columns::SESSION])
                .where_(Where::And(vec![
                    Where::Eq(session_step::entity::columns::EX_CAT, category.into()),
                    Where::Eq(session_step::entity::columns::EX_ID, id.into()),
                ]))
                .to_subquery(),
        ));

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
                    row_int(s, session::entity::columns::DATE.as_ref()) as u32,
                    row_text(s, session::entity::columns::NAME.as_ref()),
                )
            })
            .collect();

        let body_metrics = BodyMetricRepository::select()
            .order_by(OrderBy::Desc(body_metric::entity::columns::DATE))
            .fetch_in(conn)?;

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

        res.chart_session = chart_session;
    }

    Ok(res)
}

pub async fn get_exercises_catalog<R: Runtime>(
    app: &AppHandle<R>,
) -> Result<HashMap<u16, Vec<u16>>, String> {
    run_blocking(app, |db| {
        let mut catalog: HashMap<u16, Vec<u16>> = HashMap::new();

        db.run_in_connection(|conn| Ok(ExerciseRepository::select().fetch_in(conn)?))?
            .iter()
            .for_each(|e| {
                catalog.entry(e.category).or_default().push(e.id);
            });

        Ok(catalog)
    })
    .await
    .map_err(|e: DatabaseError| e.to_string())
}

#[cfg(debug_assertions)]
pub fn update_all_prs(
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
            Some((
                match r.get(session_step::entity::columns::EX_CAT.as_ref()) {
                    Some(Value::Int64(val)) => *val as u16,
                    _ => return None,
                },
                match r.get(session_step::entity::columns::EX_ID.as_ref()) {
                    Some(Value::Int64(val)) => *val as u16,
                    _ => return None,
                },
            ))
        })
        .for_each(|e| {
            exercises.insert(e);
        });
    update_prs(tx, &exercises, &[], None)
}

/// Recomputes the `pr` flag for each affected exercise and notifies if any of the just-imported/edited sessions set a new record.
pub fn update_prs(
    tx: &rusqlite_orm::rusqlite::Transaction,
    exercises: &HashSet<(u16, u16)>,
    sessions: &[u32],
    lang: Option<Languages>,
) -> rusqlite_orm::errors::Result<()> {
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

    if new_prs && let Some(lang) = lang {
        show_notification(NotificationDefinition {
            title: translate("new_record", lang),
            body: translate("contratulations_pr", lang),
            kind: NotificationKind::Temporal,
        });
    }

    Ok(())
}

pub async fn set_exercise_unilateral<R: Runtime>(
    app: &AppHandle<R>,
    settings: &SettingsLock,
    category: u16,
    id: u16,
    value: bool,
) -> Result<(), String> {
    let res: Result<_, String> = run_blocking(app, move |database| {
        ExerciseRepository::update()
            .set(exercise::entity::columns::UNILATERAL, value.into())
            .where_(Where::And(vec![
                Where::Eq(exercise::entity::columns::CATEGORY, category.into()),
                Where::Eq(exercise::entity::columns::ID, id.into()),
            ]))
            .execute(database)
            .map_err(|e| e.to_string())?;

        Ok(())
    })
    .await;

    match res {
        Ok(l) => {
            info!("Updated unilateral flag for exercise");
            Ok(l)
        }
        Err(e) => Err(report_error(
            e,
            settings.read().unwrap().language,
            "error_updating_exercise",
            "Error updating unilateral flag for exercise",
        )),
    }
}

pub async fn set_exercise_progression<R: Runtime>(
    app: &AppHandle<R>,
    settings: &SettingsLock,
    category: u16,
    id: u16,
    value: bool,
) -> Result<(), String> {
    let res: Result<_, String> = run_blocking(app, move |database| {
        ExerciseRepository::update()
            .set(exercise::entity::columns::PROGRESSION, value.into())
            .where_(Where::And(vec![
                Where::Eq(exercise::entity::columns::CATEGORY, category.into()),
                Where::Eq(exercise::entity::columns::ID, id.into()),
            ]))
            .execute(database)
            .map_err(|e| e.to_string())?;

        Ok(())
    })
    .await;

    match res {
        Ok(l) => {
            info!("Updated progression flag for exercise");
            Ok(l)
        }
        Err(e) => Err(report_error(
            e,
            settings.read().unwrap().language,
            "error_updating_exercise",
            "Error updating progression flag for exercise",
        )),
    }
}

pub async fn set_exercise_progression_parameters<R: Runtime>(
    app: &AppHandle<R>,
    settings: &SettingsLock,
    category: u16,
    id: u16,
    reps: &str,
    weight: f32,
) -> Result<(), String> {
    let reps = reps.to_string();
    let res: Result<_, String> = run_blocking(app, move |database| {
        ExerciseRepository::update()
            .set(exercise::entity::columns::REPS, reps.into())
            .set(exercise::entity::columns::WEIGHT, weight.into())
            .where_(Where::And(vec![
                Where::Eq(exercise::entity::columns::CATEGORY, category.into()),
                Where::Eq(exercise::entity::columns::ID, id.into()),
            ]))
            .execute(database)
            .map_err(|e| e.to_string())?;

        Ok(())
    })
    .await;

    match res {
        Ok(l) => {
            info!("Updated rep ranges for exercise");
            Ok(l)
        }
        Err(e) => Err(report_error(
            e,
            settings.read().unwrap().language,
            "error_updating_exercise",
            "Error updating rep ranges for exercise",
        )),
    }
}

pub async fn get_progression_status<R: Runtime>(
    app: &AppHandle<R>,
    category: u16,
    id: u16,
) -> Result<Option<ProgressionSuggestion>, String> {
    run_blocking(app, move |database| {
        database.run_in_connection(|conn| {
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
                .fetch_one_in(conn)?
                .map(|r| ProgressionSuggestion {
                    reps: row_int_nullable(&r, workout_step::entity::columns::REPS.as_ref())
                        .unwrap_or_default() as u16,
                    weight: row_f64_nullable(&r, workout_step::entity::columns::WEIGHT.as_ref())
                        .unwrap_or_default() as f32,
                }))
        })
    })
    .await
    .map_err(|e: DatabaseError| e.to_string())
}

pub async fn get_progression_suggestion<R: Runtime>(
    app: &AppHandle<R>,
    category: u16,
    id: u16,
) -> Result<Option<ProgressionSuggestion>, String> {
    run_blocking(app, move |database| {
        database.run_in_connection(|conn| Ok(fetch_progression_suggestion(conn, category, id)?))
    })
    .await
    .map_err(|e: DatabaseError| e.to_string())
}

pub fn fetch_progression_suggestion(
    conn: &rusqlite_orm::rusqlite::Connection,
    category: u16,
    id: u16,
) -> rusqlite_orm::errors::Result<Option<ProgressionSuggestion>> {
    let Some(exercise) = ExerciseRepository::select_by_id_in(conn, category, id)? else {
        return Ok(None);
    };

    if !exercise.progression {
        return Ok(None);
    }

    let latest_subquery = SessionStepRepository::select()
        .columns(&[session_step::entity::columns::SESSION])
        .where_(Where::And(vec![
            Where::Eq(session_step::entity::columns::EX_CAT, category.into()),
            Where::Eq(session_step::entity::columns::EX_ID, id.into()),
            Where::Eq(
                session_step::entity::columns::KIND,
                StepType::Exercise.into(),
            ),
        ]))
        .order_by(OrderBy::Desc(session_step::entity::columns::SESSION))
        .limit(1)
        .to_subquery();

    let sets: Vec<_> = SessionStepRepository::select()
        .columns(&[
            session_step::entity::columns::WEIGHT,
            session_step::entity::columns::REPS,
        ])
        .where_(Where::And(vec![
            Where::Eq(session_step::entity::columns::EX_CAT, category.into()),
            Where::Eq(session_step::entity::columns::EX_ID, id.into()),
            Where::Eq(
                session_step::entity::columns::KIND,
                StepType::Exercise.into(),
            ),
            Where::EqSub(session_step::entity::columns::SESSION, latest_subquery),
        ]))
        .fetch_in(conn)?
        .iter()
        .map(|r| {
            (
                row_f64_nullable(r, session_step::entity::columns::WEIGHT.as_ref())
                    .unwrap_or_default() as f32,
                row_int_nullable(r, session_step::entity::columns::REPS.as_ref())
                    .unwrap_or_default() as u16,
            )
        })
        .collect();

    let Some(&(weight, _)) = sets.first() else {
        return Ok(None);
    };
    if sets.iter().any(|&(w, _)| w != weight) {
        return Ok(None);
    }

    let range: Vec<_> = exercise
        .reps
        .split('-')
        .filter_map(|s| s.parse::<u16>().ok())
        .collect();
    if range.is_empty() {
        return Ok(None);
    }

    // Index of the highest range step reached by each set; every set must reach the range and the weakest one rules
    let brackets: Option<Vec<_>> = sets
        .iter()
        .map(|&(_, reps)| {
            let reps = if exercise.unilateral { reps / 2 } else { reps };
            range.iter().rposition(|&r| reps >= r)
        })
        .collect();
    let Some(bracket) = brackets.and_then(|b| b.into_iter().min()) else {
        return Ok(None);
    };

    let (weight, reps) = match bracket {
        i if i == range.len() - 1 => (weight + exercise.weight, range[0]),
        i => (weight, range[i + 1]),
    };
    let suggestion = ProgressionSuggestion {
        weight,
        reps: if exercise.unilateral { reps * 2 } else { reps },
    };

    // Steps of disabled workouts don't count: they alone would suggest what is already programmed
    let pending = WorkoutStepRepository::select()
        .where_(Where::And(vec![
            Where::Eq(workout_step::entity::columns::EX_CAT, category.into()),
            Where::Eq(workout_step::entity::columns::EX_ID, id.into()),
            exercise_step(),
            Where::NotNull(workout_step::entity::columns::REPS),
            below_suggestion(&suggestion),
            Where::InSub(
                workout_step::entity::columns::WORKOUT,
                WorkoutRepository::select()
                    .columns(&[workout::entity::columns::NAME])
                    .where_(Where::Eq(workout::entity::columns::ENABLED, true.into()))
                    .to_subquery(),
            ),
        ]))
        .count_in(conn)?;

    Ok((pending > 0).then_some(suggestion))
}

/// Only exercise steps progress: warm-ups and the like keep their own load.
fn exercise_step() -> Where<WorkoutStep> {
    Where::Eq(
        workout_step::entity::columns::KIND,
        StepType::Exercise.into(),
    )
}

/// Steps programmed below `suggestion`; steps already at or above it are never lowered.
fn below_suggestion(suggestion: &ProgressionSuggestion) -> Where<WorkoutStep> {
    Where::Or(vec![
        weight_below(suggestion.weight),
        Where::And(vec![
            weight_equals(suggestion.weight),
            Where::Lt(workout_step::entity::columns::REPS, suggestion.reps.into()),
        ]),
    ])
}

/// Steps loaded below `weight`, a missing load counting as 0.
fn weight_below(weight: f32) -> Where<WorkoutStep> {
    let lt = Where::Lt(workout_step::entity::columns::WEIGHT, weight.into());
    if weight > 0.0 {
        Where::Or(vec![Where::Null(workout_step::entity::columns::WEIGHT), lt])
    } else {
        lt
    }
}

/// Steps loaded with exactly `weight`, a missing load counting as 0.
fn weight_equals(weight: f32) -> Where<WorkoutStep> {
    let eq = Where::Eq(workout_step::entity::columns::WEIGHT, weight.into());
    if weight == 0.0 {
        Where::Or(vec![Where::Null(workout_step::entity::columns::WEIGHT), eq])
    } else {
        eq
    }
}

pub async fn apply_progression_suggestion<R: Runtime>(
    app: &AppHandle<R>,
    settings: &SettingsLock,
    connect: &ConnectLock,
    category: u16,
    id: u16,
    suggestion: ProgressionSuggestion,
) -> Result<(), String> {
    let res = run_blocking(app, move |database| {
        database
            .run_in_transaction(|tx| {
                // Reps go first: the weight update would otherwise change which steps are below
                let mut modified = WorkoutStepRepository::update()
                    .set(workout_step::entity::columns::REPS, suggestion.reps.into())
                    .where_(Where::And(vec![
                        Where::Eq(workout_step::entity::columns::EX_CAT, category.into()),
                        Where::Eq(workout_step::entity::columns::EX_ID, id.into()),
                        exercise_step(),
                        Where::NotNull(workout_step::entity::columns::REPS),
                        below_suggestion(&suggestion),
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
                        .filter_map(|r| {
                            match r.get(workout_step::entity::columns::WORKOUT.as_ref()) {
                                Some(Value::Text(name)) => Some(name.clone()),
                                _ => None,
                            }
                        })
                        .collect();
                    WorkoutDeviceRepository::update()
                        .set(workout_device::entity::columns::PENDING, true.into())
                        .where_(Where::InSub(
                            workout_device::entity::columns::WORKOUT,
                            WorkoutStepRepository::select()
                                .distinct(&[workout_step::entity::columns::WORKOUT])
                                .where_(Where::And(vec![
                                    Where::Eq(
                                        workout_step::entity::columns::EX_CAT,
                                        category.into(),
                                    ),
                                    Where::Eq(workout_step::entity::columns::EX_ID, id.into()),
                                    exercise_step(),
                                ]))
                                .to_subquery(),
                        ))
                        .execute_in(tx)?;
                }

                Ok((modified > 0, workouts))
            })
            .map_err(|e| e.to_string())
    })
    .await;

    let lang = settings.read().unwrap().language;
    match res {
        Ok((modified, workouts)) => {
            info!("Applied suggested progression");
            if connect.lock().await.logged_user().is_some() {
                for wkt in &workouts {
                    let _ = send_workout_connect(app, connect, settings, wkt).await;
                }
            }
            if modified {
                push_pending_to_usb(app).await;
            }
            Ok(())
        }
        Err(e) => Err(report_error(
            e,
            lang,
            "error_applying_progression",
            "Error applying progression",
        )),
    }
}

pub async fn save_exercise_notes<R: Runtime>(
    app: &AppHandle<R>,
    settings: &SettingsLock,
    connect: &ConnectLock,
    category: u16,
    id: u16,
    notes: &str,
) -> Result<(), String> {
    let notes = if notes.trim().is_empty() {
        None
    } else {
        Some(notes.trim().to_string())
    };

    let workouts = run_blocking(app, move |database| {
        database
            .run_in_transaction(|tx| Ok(update_exercise_notes(tx, category, id, notes.clone())?))
            .map_err(|e| e.to_string())
    })
    .await?;

    if !workouts.is_empty() {
        if connect.lock().await.logged_user().is_some() {
            for wkt in &workouts {
                let _ = send_workout_connect(app, connect, settings, wkt).await;
            }
        }
        push_pending_to_usb(app).await;
    }

    Ok(())
}

/// Stores the exercise notes and queues for upload the workouts inheriting them.
/// Returns every workout inheriting the notes.
pub fn update_exercise_notes(
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
        .filter_map(
            |r| match r.get(workout_step::entity::columns::WORKOUT.as_ref()) {
                Some(Value::Text(name)) => Some(name.clone()),
                _ => None,
            },
        )
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
pub fn any_progression_suggestion(
    conn: &rusqlite_orm::rusqlite::Connection,
) -> rusqlite_orm::errors::Result<bool> {
    let exercises: Vec<(u16, u16)> = ExerciseRepository::select()
        .columns(&[
            exercise::entity::columns::CATEGORY,
            exercise::entity::columns::ID,
        ])
        .where_(Where::Eq(
            exercise::entity::columns::PROGRESSION,
            true.into(),
        ))
        .fetch_in(conn)?
        .iter()
        .map(|r| {
            (
                row_int(r, exercise::entity::columns::CATEGORY.as_ref()) as u16,
                row_int(r, exercise::entity::columns::ID.as_ref()) as u16,
            )
        })
        .collect();

    Ok(exercises.into_iter().any(|(cat, id)| {
        fetch_progression_suggestion(conn, cat, id)
            .ok()
            .flatten()
            .is_some()
    }))
}

pub async fn available_any_progression_suggestions(app: &AppHandle) -> bool {
    run_blocking(app, |database| {
        database
            .run_in_connection(|conn| Ok(any_progression_suggestion(conn)?))
            .unwrap_or(false)
    })
    .await
}

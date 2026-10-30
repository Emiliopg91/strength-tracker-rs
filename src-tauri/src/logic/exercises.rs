use std::collections::{HashMap, HashSet};

use rusqlite_orm::{
    dao::Repository,
    errors::DatabaseError,
    types::{order_by::OrderBy, where_clause::Where},
};
use strength_tracker_rs_macros::traced_command;
use tauri::{AppHandle, State};
use tauri_plugin_log::log::info;

use crate::{
    SettingsLock,
    dao::{
        exercise::{self, ExerciseRepository},
        session::{self, SessionRepository},
        set::{self, SetRepository},
        workout_device::{self, WorkoutDeviceRepository},
        workout_step::{self, WorkoutStep, WorkoutStepRepository},
    },
    dto::{
        exercises::{ExerciseDetails, ExerciseListItem, ProgressionSuggestion},
        notifications::{NotificationDefinition, NotificationKind},
        sessions::SessionSet,
    },
    logic::{
        devices::push_pending_to_connected, notifications::show_notification, report_error,
        row_f64, row_int, row_text, run_blocking,
    },
    utils::translations::{Languages, translate},
};

/// Returns every exercise in the catalog, each annotated with its current personal record.
#[traced_command]
#[tauri::command]
pub async fn get_exercises(
    app: AppHandle,
    settings: State<'_, SettingsLock>,
) -> Result<Vec<ExerciseListItem>, String> {
    info!("Getting exercises list...");
    let res = run_blocking(&app, move |database| {
        database.run_in_connection(|conn| {
            let prs = SetRepository::select()
                .where_(Where::And(vec![
                    Where::Eq(set::entity::columns::PR, true.into()),
                    Where::NotEq(set::entity::columns::EX_CAT, u16::MAX.into()),
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
                    category: pr.ex_cat,
                    id: pr.ex_id,
                    reps: pr.reps,
                    weight: pr.weight,
                    date: pr.session,
                    unilateral: exercises
                        .iter()
                        .find(|e| e.category == pr.ex_cat && e.id == pr.ex_id)
                        .unwrap()
                        .unilateral,
                    progression: suggested_progressions.contains(&(pr.ex_cat, pr.ex_id)),
                    notes: exercises
                        .iter()
                        .find(|e| e.category == pr.ex_cat && e.id == pr.ex_id)
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
#[traced_command]
#[tauri::command]
pub async fn get_exercise_details(
    app: AppHandle,
    settings: State<'_, SettingsLock>,
    category: u16,
    id: u16,
) -> Result<ExerciseDetails, String> {
    info!(
        "Getting details for exercise with category {} and id {}...",
        category, id
    );
    let res = run_blocking(&app, move |database| {
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

    let series = SetRepository::select_by_exercise_in(
        conn,
        category,
        id,
        Some(&[
            OrderBy::Desc(set::entity::columns::SESSION),
            OrderBy::Asc(set::entity::columns::IDX),
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
        res.reps = pr.reps;
        res.weight = pr.weight;
        res.pr_date = pr.session;

        let mut conditions = Vec::new();
        conditions.push(Where::InSub(
            session::entity::columns::DATE,
            SetRepository::select()
                .distinct(&[set::entity::columns::SESSION])
                .where_(Where::And(vec![
                    Where::Eq(set::entity::columns::EX_CAT, category.into()),
                    Where::Eq(set::entity::columns::EX_ID, id.into()),
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

            let entry = res.series.entry(ex_str.clone()).or_default();
            entry.push(SessionSet::from(&serie));
        }

        res.chart_session = chart_session;
    }

    Ok(res)
}

#[traced_command]
#[tauri::command]
pub async fn get_exercises_catalog(app: AppHandle) -> Result<HashMap<u16, Vec<u16>>, String> {
    run_blocking(&app, |db| {
        let mut catalog: HashMap<u16, Vec<u16>> = HashMap::new();

        db.run_in_connection(|conn| Ok(ExerciseRepository::select().fetch_in(conn)?))?
            .iter()
            .for_each(|e| {
                if e.category != u16::MAX {
                    catalog.entry(e.category).or_default().push(e.id);
                }
            });

        Ok(catalog)
    })
    .await
    .map_err(|e: DatabaseError| e.to_string())
}

/// Recomputes the `pr` flag for each affected exercise and notifies if any of the just-imported/edited sessions set a new record.
pub fn update_prs(
    tx: &rusqlite_orm::rusqlite::Transaction,
    exercises: HashSet<(u16, u16)>,
    sessions: &[u32],
    lang: Option<Languages>,
) -> rusqlite_orm::errors::Result<()> {
    let mut new_prs = false;

    let sessions = sessions.to_vec();

    let mut update_false_conditions = vec![];
    let mut update_true_conditions = vec![];

    for exer in &exercises {
        if exer.0 != u16::MAX {
            update_false_conditions.push(vec![exer.0.into(), exer.1.into()]);
            if let Some(pr) = SetRepository::select()
                .where_(Where::And(vec![
                    Where::Eq(set::entity::columns::EX_CAT, exer.0.into()),
                    Where::Eq(set::entity::columns::EX_ID, exer.1.into()),
                ]))
                .order_by(OrderBy::Desc(set::entity::columns::WEIGHT))
                .order_by(OrderBy::Desc(set::entity::columns::REPS))
                .order_by(OrderBy::Asc(set::entity::columns::SESSION))
                .order_by(OrderBy::Asc(set::entity::columns::IDX))
                .limit(1)
                .fetch_one_in(tx)?
            {
                update_true_conditions.push(vec![pr.session.into(), pr.idx.into()]);
                new_prs = new_prs || sessions.contains(&pr.session);
            }
        }
    }

    if !update_true_conditions.is_empty() {
        SetRepository::update()
            .set(set::entity::columns::PR, false.into())
            .where_(Where::And(vec![
                Where::InMultiple(
                    vec![set::entity::columns::EX_CAT, set::entity::columns::EX_ID],
                    update_false_conditions,
                ),
                Where::Eq(set::entity::columns::PR, true.into()),
            ]))
            .execute_in(tx)?;
        SetRepository::update()
            .set(set::entity::columns::PR, true.into())
            .where_(Where::InMultiple(
                vec![set::entity::columns::SESSION, set::entity::columns::IDX],
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

#[traced_command]
#[tauri::command]
pub async fn set_exercise_unilateral(
    app: AppHandle,
    settings: State<'_, SettingsLock>,
    category: u16,
    id: u16,
    value: bool,
) -> Result<(), String> {
    let res: Result<_, String> = run_blocking(&app, move |database| {
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

#[traced_command]
#[tauri::command]
pub async fn set_exercise_progression(
    app: AppHandle,
    settings: State<'_, SettingsLock>,
    category: u16,
    id: u16,
    value: bool,
) -> Result<(), String> {
    let res: Result<_, String> = run_blocking(&app, move |database| {
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

#[traced_command]
#[tauri::command]
pub async fn set_exercise_progression_parameters(
    app: AppHandle,
    settings: State<'_, SettingsLock>,
    category: u16,
    id: u16,
    reps: &str,
    weight: f32,
) -> Result<(), String> {
    let reps = reps.to_string();
    let res: Result<_, String> = run_blocking(&app, move |database| {
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

#[traced_command]
#[tauri::command]
pub async fn get_progression_status(
    app: AppHandle,
    category: u16,
    id: u16,
) -> Result<Option<ProgressionSuggestion>, String> {
    run_blocking(&app, move |database| {
        database.run_in_connection(|conn| {
            Ok(WorkoutStepRepository::select()
                .columns(&[
                    workout_step::entity::columns::REPS,
                    workout_step::entity::columns::WEIGHT,
                ])
                .where_(Where::And(vec![
                    Where::Eq(workout_step::entity::columns::EX_CAT, category.into()),
                    Where::Eq(workout_step::entity::columns::EX_ID, id.into()),
                ]))
                .order_by(OrderBy::Desc(workout_step::entity::columns::WEIGHT))
                .order_by(OrderBy::Desc(workout_step::entity::columns::REPS))
                .fetch_one_in(conn)?
                .map(|r| ProgressionSuggestion {
                    reps: row_int(&r, workout_step::entity::columns::REPS.as_ref()) as u16,
                    weight: row_f64(&r, workout_step::entity::columns::WEIGHT.as_ref()) as f32,
                }))
        })
    })
    .await
    .map_err(|e: DatabaseError| e.to_string())
}

#[traced_command]
#[tauri::command]
pub async fn get_progression_suggestion(
    app: AppHandle,
    category: u16,
    id: u16,
) -> Result<Option<ProgressionSuggestion>, String> {
    run_blocking(&app, move |database| {
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

    let latest_subquery = SetRepository::select()
        .columns(&[set::entity::columns::SESSION])
        .where_(Where::And(vec![
            Where::Eq(set::entity::columns::EX_CAT, category.into()),
            Where::Eq(set::entity::columns::EX_ID, id.into()),
        ]))
        .order_by(OrderBy::Desc(set::entity::columns::SESSION))
        .limit(1)
        .to_subquery();

    let sets: Vec<_> = SetRepository::select()
        .columns(&[set::entity::columns::WEIGHT, set::entity::columns::REPS])
        .where_(Where::And(vec![
            Where::Eq(set::entity::columns::EX_CAT, category.into()),
            Where::Eq(set::entity::columns::EX_ID, id.into()),
            Where::EqSub(set::entity::columns::SESSION, latest_subquery),
        ]))
        .fetch_in(conn)?
        .iter()
        .map(|r| {
            (
                row_f64(r, set::entity::columns::WEIGHT.as_ref()) as f32,
                row_int(r, set::entity::columns::REPS.as_ref()) as u16,
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

    let pending = WorkoutStepRepository::select()
        .where_(Where::And(vec![
            Where::Eq(workout_step::entity::columns::EX_CAT, category.into()),
            Where::Eq(workout_step::entity::columns::EX_ID, id.into()),
            Where::NotNull(workout_step::entity::columns::REPS),
            below_suggestion(&suggestion),
        ]))
        .count_in(conn)?;

    Ok((pending > 0).then_some(suggestion))
}

/// Steps programmed below `suggestion`; steps already at or above it are never lowered.
fn below_suggestion(suggestion: &ProgressionSuggestion) -> Where<WorkoutStep> {
    Where::Or(vec![
        Where::Lt(
            workout_step::entity::columns::WEIGHT,
            suggestion.weight.into(),
        ),
        Where::And(vec![
            Where::Eq(
                workout_step::entity::columns::WEIGHT,
                suggestion.weight.into(),
            ),
            Where::Lt(workout_step::entity::columns::REPS, suggestion.reps.into()),
        ]),
    ])
}

#[traced_command]
#[tauri::command]
pub async fn apply_progression_suggestion(
    app: AppHandle,
    settings: State<'_, SettingsLock>,
    category: u16,
    id: u16,
    suggestion: ProgressionSuggestion,
) -> Result<(), String> {
    let res = run_blocking(&app, move |database| {
        database
            .run_in_transaction(|tx| {
                // Reps go first: the weight update would otherwise change which steps are below
                let mut modified = WorkoutStepRepository::update()
                    .set(workout_step::entity::columns::REPS, suggestion.reps.into())
                    .where_(Where::And(vec![
                        Where::Eq(workout_step::entity::columns::EX_CAT, category.into()),
                        Where::Eq(workout_step::entity::columns::EX_ID, id.into()),
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
                        Where::Lt(
                            workout_step::entity::columns::WEIGHT,
                            suggestion.weight.into(),
                        ),
                    ]))
                    .execute_in(tx)?;

                if modified > 0 {
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
                                ]))
                                .to_subquery(),
                        ))
                        .execute_in(tx)?;
                }

                Ok(modified > 0)
            })
            .map_err(|e| e.to_string())
    })
    .await;

    let lang = settings.read().unwrap().language;
    match res {
        Ok(modified) => {
            info!("Applied suggested progression");
            if modified {
                push_pending_to_connected(&app).await;
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

#[traced_command]
#[tauri::command]
pub async fn save_exercise_notes(
    app: AppHandle,
    category: u16,
    id: u16,
    notes: &str,
) -> Result<(), String> {
    let notes = if notes.trim().is_empty() {
        None
    } else {
        Some(notes.trim().to_string())
    };

    run_blocking(&app, move |database| {
        database
            .run_in_transaction(|tx| {
                ExerciseRepository::update()
                    .set(exercise::entity::columns::NOTES, notes.clone().into())
                    .where_(Where::And(vec![
                        Where::Eq(exercise::entity::columns::CATEGORY, category.into()),
                        Where::Eq(exercise::entity::columns::ID, id.into()),
                    ]))
                    .execute_in(tx)?;

                Ok(())
            })
            .map_err(|e| e.to_string())?;

        Ok(())
    })
    .await
}

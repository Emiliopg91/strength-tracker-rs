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
        exercise::ExerciseRepository,
        session::{self, SessionRepository},
        set::{self, SetRepository},
    },
    dto::{
        exercises::{ExerciseDetails, ExerciseListItem},
        notifications::{NotificationDefinition, NotificationKind},
        sessions::SessionSet,
    },
    logic::{notifications::show_notification, report_error, row_int, row_text, run_blocking},
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
    let res = run_blocking(app, move |database| {
        database.run_in_connection(|conn| {
            let prs = SetRepository::select()
                .where_(Where::And(vec![
                    Where::Eq(set::entity::columns::PR, true.into()),
                    Where::NotEq(set::entity::columns::EX_CAT, u16::MAX.into()),
                ]))
                .fetch_in(conn)?;

            Ok(prs
                .iter()
                .map(|pr| ExerciseListItem {
                    category: pr.ex_cat,
                    id: pr.ex_id,
                    reps: pr.reps,
                    weight: pr.weight,
                    date: pr.session,
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
    let res = run_blocking(app, move |database| {
        database.run_in_connection(|conn| {
            let exercise = ExerciseRepository::select_by_id_in(conn, category, id)?.unwrap();
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

            let pr = series.iter().find(|s| s.pr).unwrap();
            res.reps = pr.reps;
            res.weight = pr.weight;
            res.pr_date = pr.session;

            let workouts = SessionRepository::select()
                .columns(&[
                    session::entity::columns::DATE,
                    session::entity::columns::NAME,
                ])
                .where_(Where::InSub(
                    session::entity::columns::DATE,
                    SetRepository::select()
                        .distinct(&[set::entity::columns::SESSION])
                        .where_(Where::And(vec![
                            Where::Eq(set::entity::columns::EX_CAT, category.into()),
                            Where::Eq(set::entity::columns::EX_ID, id.into()),
                        ]))
                        .to_subquery(),
                ))
                .fetch_in(conn)?
                .iter()
                .map(|s| {
                    (
                        row_int(s, session::entity::columns::DATE.as_ref()) as u32,
                        row_text(s, session::entity::columns::NAME.as_ref()),
                    )
                })
                .collect::<HashMap<_, _>>();

            let mut last_session = None;
            let mut ex_str = String::new();
            for serie in series {
                if last_session != Some(serie.session) {
                    ex_str = format!(
                        "{}\n{}",
                        workouts.get(&serie.session).unwrap(),
                        serie.session
                    );
                    res.workouts.push(ex_str.clone());
                    last_session = Some(serie.session);
                }

                let entry = res.series.entry(ex_str.clone()).or_default();
                entry.push(SessionSet::from(&serie));
            }

            Ok(res)
        })
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

#[traced_command]
#[tauri::command]
pub async fn get_exercises_catalog(app: AppHandle) -> Result<HashMap<u16, Vec<u16>>, String> {
    run_blocking(app, |db| {
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

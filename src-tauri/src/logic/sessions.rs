use std::{
    collections::{HashMap, HashSet},
    fs::{self},
    path::{Path, PathBuf},
    sync::Mutex,
    time::Duration,
};

use crate::{
    SettingsLock,
    dao::{
        additional_data::{self, AdditionalData, AdditionalDataRepository},
        device::Device,
        exercise::{self, ExerciseRepository},
        lap::LapRepository,
        session::{self, Session, SessionRepository},
        set::{self, SetRepository, entity},
        workout::{self, Workout, WorkoutRepository},
        workout_step::WorkoutStepRepository,
    },
    dto::{
        notifications::{NotificationDefinition, NotificationKind},
        sessions::{SessionDetails, SessionListItem, SessionLocation, SessionSetsUpdate},
    },
    fit::parser::FitParser,
    logic::{
        exercises::update_prs, notifications::show_notification, report_error, row_int,
        run_blocking,
    },
    utils::translations::{Languages, translate, translate_and_replace},
};
use chrono::{Datelike, Days, Local, Months, TimeZone, offset::LocalResult};
use curl_rest::StatusCode;
use rayon::prelude::*;
use rusqlite_orm::{
    dao::Repository,
    database::DatabasePool,
    errors::DatabaseError,
    types::{order_by::OrderBy, value::Value, where_clause::Where},
};
use strength_tracker_rs_macros::traced_command;
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_log::log::{error, info, warn};
use tokio::process::Command;

/// Returns every recorded session, newest first.
#[traced_command]
#[tauri::command]
pub async fn get_sessions(
    app: AppHandle,
    settings: State<'_, SettingsLock>,
    limit: Option<i32>,
) -> Result<Vec<SessionListItem>, String> {
    info!("Getting sessions list...");
    let res = run_blocking(app, move |database| {
        database.run_in_connection(|conn| Ok(fetch_sessions(conn, limit, None, None, None)?))
    })
    .await;

    match res {
        Ok(l) => {
            info!("Retreived {} sessions", l.len());
            Ok(l)
        }
        Err(e) => Err(report_error(
            e,
            settings.read().unwrap().language,
            "error_session_list",
            "Error getting sessions list",
        )),
    }
}

/// Lists sessions newest first (from `min_date` on, if given), flagged with whether they have sets and records.
pub fn fetch_sessions(
    conn: &rusqlite_orm::rusqlite::Connection,
    from: Option<i32>,
    to: Option<i32>,
    sport: Option<u8>,
    sub_sport: Option<u8>,
) -> rusqlite_orm::errors::Result<Vec<SessionListItem>> {
    let mut select_builder =
        SessionRepository::select().order_by(OrderBy::Desc(session::entity::columns::DATE));

    let mut conditions = Vec::new();
    if let Some(min_date) = from {
        conditions.push(Where::Gte(session::entity::columns::DATE, min_date.into()))
    }

    if let Some(max_date) = to {
        conditions.push(Where::Lte(session::entity::columns::DATE, max_date.into()))
    }

    if let Some(sport) = sport {
        conditions.push(Where::Eq(session::entity::columns::SPORT, sport.into()))
    }

    if let Some(sub_sport) = sub_sport {
        conditions.push(Where::Eq(
            session::entity::columns::SUB_SPORT,
            sub_sport.into(),
        ))
    }

    if !conditions.is_empty() {
        select_builder = select_builder.where_(Where::And(conditions))
    }

    let sessions = select_builder.fetch_in(conn)?;

    let set_flags = session_set_flags(conn, from, to)?;

    Ok(sessions
        .into_iter()
        .map(|s| {
            let mut r = SessionListItem::from(&s);
            if let Some(has_record) = set_flags.get(&s.date) {
                r.has_sets = true;
                r.has_record = *has_record;
            }
            r
        })
        .collect())
}

/// Maps every session that has sets (within `[min_date, max_date]`, each bound optional) to whether any of them is a personal record.
pub fn session_set_flags(
    conn: &rusqlite_orm::rusqlite::Connection,
    min_date: Option<i32>,
    max_date: Option<i32>,
) -> rusqlite_orm::errors::Result<HashMap<u32, bool>> {
    let mut select_builder = SetRepository::select()
        .distinct(&[set::entity::columns::SESSION, set::entity::columns::PR]);

    // `where_` replaces the previous condition, so bounds must be combined in a single `And`
    let mut conditions = Vec::new();
    if let Some(min_date) = min_date {
        conditions.push(Where::Gte(set::entity::columns::SESSION, min_date.into()));
    }
    if let Some(max_date) = max_date {
        conditions.push(Where::Lte(set::entity::columns::SESSION, max_date.into()));
    }
    if !conditions.is_empty() {
        select_builder = select_builder.where_(Where::And(conditions));
    }

    let mut flags: HashMap<u32, bool> = HashMap::new();
    for row in select_builder.fetch_in(conn)? {
        let session = row_int(&row, set::entity::columns::SESSION.as_ref()) as u32;
        let pr = row_int(&row, set::entity::columns::PR.as_ref()) != 0;
        *flags.entry(session).or_default() |= pr;
    }
    Ok(flags)
}

/// Returns full details for one session (series grouped by exercise, heart rate, GPS, speeds, device).
#[traced_command]
#[tauri::command]
pub async fn get_session_details(
    app: AppHandle,
    settings: State<'_, SettingsLock>,
    timestamp: u32,
) -> Result<SessionDetails, String> {
    info!(
        "Getting details for session {}",
        Local
            .timestamp_opt(timestamp as i64, 0)
            .unwrap()
            .to_rfc3339()
    );

    let res = run_blocking(app, move |database| {
        database.run_in_connection(|conn| Ok(fetch_session_details(conn, timestamp, true, true)?))
    })
    .await;

    match res {
        Ok(details) => {
            info!("Found details for session {}", details.timestamp);
            Ok(details)
        }
        Err(e) => Err(report_error(
            e,
            settings.read().unwrap().language,
            "error_session_details",
            "Error getting session details",
        )),
    }
}

/// Loads one session with its sets (sorted), device, additional data and laps (sorted).
pub fn fetch_session_details(
    conn: &rusqlite_orm::rusqlite::Connection,
    timestamp: u32,
    with_add_data: bool,
    with_laps: bool,
) -> rusqlite_orm::errors::Result<SessionDetails> {
    let mut session = SessionRepository::select_by_id_in(conn, timestamp)?.unwrap();

    session.fetch_sets_relationship_in(conn)?;

    if session.device.is_some() {
        session.fetch_device_obj_relationship_in(conn)?;
    }

    if with_add_data {
        session.fetch_additional_data_relationship_in(conn)?;
    }

    if with_laps {
        session.fetch_laps_relationship_in(conn)?;
    }

    let condition_set: HashSet<(_, _)> = session.sets.iter().map(|r| (r.ex_cat, r.ex_id)).collect();

    let in_conditions: Vec<Vec<Value>> = condition_set
        .into_iter()
        .map(|(cat, id)| vec![cat.into(), id.into()])
        .collect();

    let exercises = if in_conditions.is_empty() {
        Vec::new()
    } else {
        ExerciseRepository::select()
            .where_(Where::InMultiple(
                vec![
                    exercise::entity::columns::CATEGORY,
                    exercise::entity::columns::ID,
                ],
                in_conditions,
            ))
            .fetch_in(conn)?
    };
    session.sets.sort_by_key(|s| s.idx);
    session.laps.sort_by_key(|l| l.idx);

    Ok(SessionDetails::from((
        &session,
        exercises.as_slice(),
        session.sets.as_slice(),
        session.laps.as_slice(),
    )))
}

/// Applies user edits (session notes and reps/weight/exercise of its series) and recomputes personal records.
#[traced_command]
#[tauri::command]
pub async fn save_session_changes(
    app: AppHandle,
    settings: State<'_, SettingsLock>,
    details: SessionSetsUpdate,
) -> Result<(), String> {
    info!(
        "Saving changes on session {}...",
        Local
            .timestamp_opt(details.timestamp as i64, 0)
            .unwrap()
            .to_rfc3339()
    );
    let lang = settings.read().unwrap().language;

    let res = run_blocking(app, move |database| {
        database.run_in_transaction(|tx| {
            update_session_notes(tx, details.timestamp, details.notes.as_deref())?;

            let mut exercises = HashSet::new();

            SetRepository::select()
                .columns(&[entity::columns::EX_CAT, entity::columns::EX_ID])
                .where_(Where::And(vec![
                    Where::Eq(entity::columns::SESSION, details.timestamp.into()),
                    Where::In(
                        entity::columns::IDX,
                        details.sets.iter().map(|s| s.idx.into()).collect(),
                    ),
                ]))
                .fetch_in(tx)?
                .iter()
                .for_each(|row| {
                    exercises.insert((
                        row_int(row, entity::columns::EX_CAT.as_ref()) as u16,
                        row_int(row, entity::columns::EX_ID.as_ref()) as u16,
                    ));
                });

            for serie in &details.sets {
                SetRepository::update()
                    .set(entity::columns::REPS, serie.reps.into())
                    .set(entity::columns::WEIGHT, serie.weight.into())
                    .set(entity::columns::EX_CAT, serie.ex_cat.into())
                    .set(entity::columns::EX_ID, serie.ex_id.into())
                    .where_(Where::And(vec![
                        Where::Eq(entity::columns::SESSION, details.timestamp.into()),
                        Where::Eq(entity::columns::IDX, serie.idx.into()),
                    ]))
                    .execute_in(tx)?;
                exercises.insert((serie.ex_cat, serie.ex_id));
            }

            update_prs(tx, exercises, &[details.timestamp], Some(lang))?;
            Ok(())
        })
    })
    .await;

    match res {
        Ok(l) => {
            info!("Session updated succesfully");
            show_notification(NotificationDefinition {
                title: translate("ok_update_session", lang),
                body: "".to_string(),
                kind: NotificationKind::Temporal,
            });

            Ok(l)
        }
        Err(e) => Err(report_error(
            e,
            lang,
            "error_update_session",
            "Error updating session",
        )),
    }
}

/// Tauri command wrapper around `_import_from_device`; returns the number of sessions imported.
#[traced_command]
#[tauri::command]
pub async fn import_from_files(
    app: AppHandle,
    settings: State<'_, SettingsLock>,
) -> Result<usize, String> {
    let lang = { settings.read().unwrap().language };
    let output = Command::new("zenity")
        .args([
            "--file-selection",
            "--multiple",
            "--separator=\n",
            "--file-filter=FIT (*.fit) | *.fit",
            &format!("--title={}", translate("select_files_import", lang)),
        ])
        .output()
        .await
        .map_err(|e| e.to_string())?;

    let files = if output.status.success() {
        String::from_utf8(output.stdout)
            .map_err(|e| e.to_string())?
            .lines()
            .map(PathBuf::from)
            .collect()
    } else {
        Vec::new()
    };

    if !files.is_empty() {
        let res = tokio::task::spawn_blocking(move || _import_from_files(app, files.as_slice()))
            .await
            .map_err(|e| e.to_string())
            .flatten();
        return res;
    }

    Ok(0)
}

pub fn _import_from_files(app: AppHandle, files: &[PathBuf]) -> Result<usize, String> {
    let lang = app.state::<SettingsLock>().read().unwrap().language;
    let database = app.state::<DatabasePool>();
    let res = database
        .run_in_transaction(|tx| Ok(import_file_list(tx, files, None, lang, false)?))
        .map_err(|e| e.to_string());

    match res {
        Ok(res) => {
            if !res.is_empty() {
                let app = app.clone();
                std::thread::spawn(move || {
                    let db = app.state::<DatabasePool>();
                    update_pending_geolocation(&app, &db);
                });
            }
            Ok(res.len())
        }
        Err(e) => Err(report_error(
            e,
            lang,
            "error_import_sessions",
            "Error importing sessions",
        )),
    }
}

/// Parses a batch of `.FIT` files in parallel and inserts each new session (plus its exercises/series/heart rate/GPS/speeds) in the given transaction, then refreshes personal records.
pub fn import_file_list<F>(
    tx: &mut rusqlite_orm::rusqlite::Transaction,
    files: &[F],
    device: Option<Device>,
    lang: Languages,
    delete: bool,
) -> Result<Vec<u32>, DatabaseError>
where
    F: AsRef<Path> + Sync,
{
    let mut success = Vec::new();
    let mut handled_exercises = HashSet::new();

    let mut sessions: Vec<_> = files
        .par_iter()
        .filter_map(|file| {
            info!("Parsing session file {}", file.as_ref().display());
            let res = match FitParser::from_file(file) {
                Ok(parser) => match Session::try_from(parser) {
                    Ok(session) => Ok((session, file)),
                    Err(e) => Err(e),
                },
                Err(e) => Err(e),
            };
            match res {
                Ok(result) => Some(result),
                Err(e) => {
                    error!(
                        "Error parsing session file {}: {}",
                        file.as_ref().display(),
                        e
                    );

                    show_notification(NotificationDefinition {
                        title: format!("{}", file.as_ref().file_name().unwrap().display()),
                        body: translate_and_replace(
                            "error_parsing_session",
                            &[&e.to_string()],
                            lang,
                        ),
                        kind: NotificationKind::Persistant,
                    });

                    None
                }
            }
        })
        .collect();

    sessions.sort_by_key(|s| s.0.date);

    let mut existing_sessions = if sessions.is_empty() {
        HashSet::new()
    } else {
        SessionRepository::select()
            .columns(&[session::entity::columns::DATE])
            .where_(Where::In(
                session::entity::columns::DATE,
                sessions.iter().map(|(s, _)| s.date.into()).collect(),
            ))
            .fetch_in(tx)?
            .iter()
            .map(|row| row_int(row, session::entity::columns::DATE.as_ref()) as u32)
            .collect()
    };
    let mut handled_workouts = HashSet::new();

    for (session, file) in &mut sessions {
        let formatted_time = match Local.timestamp_opt(session.date as i64, 0) {
            LocalResult::Single(fecha) => fecha.format("%H:%M:%S %d/%m/%Y").to_string(),
            _ => "".to_string(),
        };

        let res: Result<bool, String> = {
            info!("Importing session {} - {}...", session.name, formatted_time);
            let added = if existing_sessions.contains(&session.date) {
                let msg = format!("Session with date {} already exists", session.date);
                warn!("{}", msg);
                false
            } else {
                let date = session.date;
                let mut series = std::mem::take(&mut session.sets);
                let mut laps = std::mem::take(&mut session.laps);
                let add_data = session.additional_data.take();

                if let Some(workout) = &session.workout_obj {
                    let updated = WorkoutRepository::update()
                        .set(
                            workout::entity::columns::NOTES,
                            workout.notes.clone().into(),
                        )
                        .where_(Where::Eq(
                            workout::entity::columns::NAME,
                            workout.name.clone().into(),
                        ))
                        .execute_in(tx)?;
                    if updated == 0 {
                        WorkoutRepository::insert()
                            .or_ignore()
                            .item(&mut Workout {
                                name: workout.name.clone(),
                                enabled: true,
                                notes: workout.notes.clone(),
                                steps: Vec::new(),
                            })
                            .execute_in(tx)?;
                    }

                    if handled_workouts.insert(workout.name.clone())
                        && WorkoutStepRepository::count_by_workout_in(tx, &workout.name)? == 0
                    {
                        let mut insert = WorkoutStepRepository::insert();
                        let mut steps = workout.steps.clone();
                        for step in &mut steps {
                            insert = insert.item(step);
                        }

                        insert.execute_in(tx)?;
                    }
                }

                if let Some(ref device) = device {
                    session.device = Some(device.serial.clone());
                }
                SessionRepository::insert().item(session).execute_in(tx)?;

                for serie in &series {
                    handled_exercises.insert((serie.ex_cat, serie.ex_id));
                }

                let mut insert = SetRepository::insert();
                let mut count = 0;
                for serie in &mut series {
                    insert = insert.item(serie);
                    count += 1;
                }
                if count > 0 {
                    insert.execute_in(tx)?;
                }

                if let Some(mut additional_data) = add_data {
                    AdditionalDataRepository::insert()
                        .item(&mut additional_data)
                        .execute_in(tx)?;
                }

                if !laps.is_empty() {
                    let mut insert = LapRepository::insert();
                    for lap in &mut laps {
                        insert = insert.item(lap)
                    }
                    insert.execute_in(tx)?;
                }

                existing_sessions.insert(date);
                success.push(date);

                if delete {
                    let _ = fs::remove_file(&file);
                }
                #[cfg(debug_assertions)]
                {
                    use std::path::PathBuf;

                    let file = PathBuf::from(format!("{}.txt", file.as_ref().display()));
                    let _ = fs::remove_file(file);
                }

                true
            };

            Ok(added)
        };

        if let Err(e) = res {
            error!("  {}", e);

            show_notification(NotificationDefinition {
                title: formatted_time,
                body: e,
                kind: NotificationKind::Persistant,
            });
        }
    }

    if !success.is_empty() {
        show_notification(NotificationDefinition {
            title: translate("ok_import_sessions", lang),
            body: translate_and_replace("imported_n_sessions", &[&success.len().to_string()], lang),
            kind: NotificationKind::Temporal,
        });
        update_prs(tx, handled_exercises, &success, Some(lang))?;
    }

    Ok(success)
}

/// Stores the notes of a single session, creating its additional data row if needed. Blank notes are stored as `NULL`.
pub fn update_session_notes(
    tx: &rusqlite_orm::rusqlite::Transaction,
    session: u32,
    notes: Option<&str>,
) -> rusqlite_orm::errors::Result<()> {
    let notes = notes.filter(|n| !n.trim().is_empty()).map(str::to_string);

    let updated = AdditionalDataRepository::update()
        .set(
            additional_data::entity::columns::NOTES,
            notes.clone().into(),
        )
        .where_(Where::Eq(
            additional_data::entity::columns::SESSION,
            session.into(),
        ))
        .execute_in(tx)?;

    if updated == 0 && notes.is_some() {
        let mut add_data = AdditionalData::new(session);
        add_data.notes = notes;
        AdditionalDataRepository::insert()
            .item(&mut add_data)
            .execute_in(tx)?;
    }

    Ok(())
}

static GELOCATION_MUTEX: Mutex<bool> = Mutex::new(false);

/// Recover all pending workouts pending on geolocation
pub fn update_pending_geolocation(app: &AppHandle, db: &DatabasePool) {
    let _lock = GELOCATION_MUTEX.lock().unwrap();
    info!("Looking for pending geocode workouts...");
    match db.run_in_connection(|conn| {
        let unnamed_sessions = SessionRepository::select()
            .columns(&[session::entity::columns::DATE])
            .where_(Where::Eq(session::entity::columns::NAME, "".into()))
            .to_subquery();

        let pending: Vec<_> = AdditionalDataRepository::select()
            .columns(&[
                additional_data::entity::columns::SESSION,
                additional_data::entity::columns::COORDINATES,
            ])
            .where_(Where::And(vec![
                Where::InSub(additional_data::entity::columns::SESSION, unnamed_sessions),
                Where::NotNull(additional_data::entity::columns::COORDINATES),
            ]))
            .order_by(OrderBy::Desc(additional_data::entity::columns::SESSION))
            .fetch_in(conn)?
            .into_iter()
            .map(|row| {
                let mut add_data = AdditionalData::new(row_int(
                    &row,
                    additional_data::entity::columns::SESSION.as_ref(),
                ) as u32);
                if let Some(Value::Blob(coordinates)) =
                    row.get(additional_data::entity::columns::COORDINATES.as_ref())
                {
                    add_data.coordinates = Some(coordinates.clone());
                }
                add_data
            })
            .collect();

        Ok(pending)
    }) {
        Ok(pendings) => {
            info!("Found {} pending sessions", pendings.len());
            for pending in pendings {
                let first = pending
                    .get_coordinates_degrees()
                    .unwrap()
                    .iter()
                    .find(|e| e.is_some())
                    .unwrap()
                    .unwrap();

                let url = format!(
                    "https://nominatim.openstreetmap.org/reverse?format=json&lat={}&lon={}",
                    first.0, first.1
                );
                //
                let resp = curl_rest::Client::with_user_agent("strength-tracker-rs")
                    .get()
                    .header(curl_rest::Header::Accept("application/json".into()))
                    .send(&url);

                match resp {
                    Ok(response) => {
                        if response.status == StatusCode::Ok {
                            match serde_json::from_slice::<serde_json::Value>(&response.body) {
                                Ok(e) => {
                                    if let Some(address) = e.get("address") {
                                        for key in ["village", "town", "city", "state", "country"] {
                                            if let Some(value) = address.get(key) {
                                                let location = value.to_string().replace("\"", "");

                                                match SessionRepository::update()
                                                    .set(
                                                        session::entity::columns::NAME,
                                                        location.clone().into(),
                                                    )
                                                    .where_(Where::Eq(
                                                        session::entity::columns::DATE,
                                                        pending.session.into(),
                                                    ))
                                                    .execute(db)
                                                {
                                                    Ok(1) => {
                                                        info!(
                                                            "Updated location for session @ {}",
                                                            pending.session
                                                        );
                                                        let payload: SessionLocation =
                                                            SessionLocation {
                                                                session: pending.session,
                                                                location,
                                                            };
                                                        let _ = app.emit(
                                                            "session_location_update",
                                                            payload,
                                                        );
                                                    }
                                                    Ok(_) => {
                                                        info!(
                                                            "Missing session @ {}",
                                                            pending.session
                                                        )
                                                    }
                                                    Err(e) => {
                                                        error!(
                                                            "Error while updating session {}: {}",
                                                            pending.session, e
                                                        )
                                                    }
                                                }

                                                break;
                                            }
                                        }
                                    }
                                }
                                Err(e) => {
                                    error!("Error parsing response: {}", e)
                                }
                            }
                        } else {
                            error!("Invalid status code: {}", response.status)
                        }
                    }
                    Err(e) => {
                        error!("Error on geocode query: {}", e)
                    }
                }

                std::thread::sleep(Duration::from_secs(1));
            }
        }
        Err(e) => {
            error!("Error while looking for pending sessions: {}", e);
        }
    }
}

#[traced_command]
#[tauri::command]
pub async fn get_heatmap_data(app: AppHandle) -> Result<Vec<Vec<(u32, u16)>>, String> {
    run_blocking(app, heatmap_data).await
}

pub fn heatmap_data(database: &DatabasePool) -> Result<Vec<Vec<(u32, u16)>>, String> {
    let today = Local::now().date_naive();
    let a_year_ago = today
        .checked_sub_months(Months::new(12))
        .and_then(|d| d.checked_add_days(Days::new(1)))
        .unwrap();
    let midnight = a_year_ago.and_hms_opt(0, 0, 0).unwrap();

    let ts = Local
        .from_local_datetime(&midnight)
        .earliest()
        .unwrap()
        .timestamp();

    let res = database
        .run_in_connection(|conn| {
            let sessions = SessionRepository::select()
                .columns(&[
                    session::entity::columns::DATE,
                    session::entity::columns::TRAINING_LOAD,
                ])
                .where_(Where::Gte(session::entity::columns::DATE, ts.into()))
                .fetch_in(conn)?;
            let prs = SetRepository::select()
                .columns(&[set::entity::columns::SESSION])
                .where_(Where::And(vec![
                    Where::Eq(set::entity::columns::PR, true.into()),
                    Where::Gte(set::entity::columns::SESSION, ts.into()),
                ]))
                .fetch_in(conn)?;

            let mut pr_counts: HashMap<u32, u16> = HashMap::new();
            for pr in &prs {
                *pr_counts
                    .entry(row_int(pr, set::entity::columns::SESSION.as_ref()) as u32)
                    .or_default() += 1;
            }

            let mut res = vec![vec![(0_u32, 0_u16); 31]; 12];
            sessions.iter().for_each(|session| {
                let date = row_int(session, session::entity::columns::DATE.as_ref()) as u32;
                let training_load =
                    row_int(session, session::entity::columns::TRAINING_LOAD.as_ref()) as u32;
                let dt = Local.timestamp_opt(date as i64, 0).single().unwrap();
                let cell = &mut res[dt.month0() as usize][dt.day0() as usize];
                cell.0 += training_load;
                cell.1 += pr_counts.get(&date).copied().unwrap_or(0);
            });

            Ok(res)
        })
        .map_err(|e| e.to_string())?;

    Ok(res)
}

use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    sync::Mutex,
    time::Duration,
};

use crate::{
    SettingsLock,
    dao::{
        additional_data::{self, AdditionalData, AdditionalDataRepository},
        device::{self, Device, DeviceRepository},
        exercise::{self, Exercise, ExerciseRepository},
        lap::{self, LapRepository},
        session::{self, Session, SessionRepository},
        session_step::{self, SessionStepRepository, entity},
        settings::WeightUnit,
        workout::{self, Workout, WorkoutRepository},
        workout_step::WorkoutStepRepository,
    },
    dto::{
        notifications::{NotificationDefinition, NotificationKind},
        sessions::{SessionDetails, SessionListItem, SessionLocation, SessionStepsUpdate},
    },
    fit::{FitFile, parser::FitParser, writer::FitWriter},
    logic::{
        ask_open_paths, ask_save_path,
        exercises::{
            fetch_exercises_muscles, fetch_progression_suggestion, progression_suggestions,
            update_prs,
        },
        notifications::show_notification,
        report_error, run_blocking, store_pending_files,
    },
    utils::{
        constants,
        translations::{Languages, translate, translate_and_replace},
    },
};
use chrono::{Datelike, Days, Local, Months, TimeZone, offset::LocalResult};
use curl_rest::StatusCode;
use rayon::{iter::Either, prelude::*};
use rusqlite_orm::{
    dao::Repository,
    database::DatabasePool,
    errors::DatabaseError,
    types::{order_by::OrderBy, value::Value, where_clause::Where},
};
use tauri::{AppHandle, Emitter, Manager, Runtime};
use tauri_plugin_log::log::{error, info, warn};

/// Returns every recorded session, newest first.
pub async fn get_sessions<R: Runtime>(
    app: &AppHandle<R>,
    settings: &SettingsLock,
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
    let mut select_builder = SessionStepRepository::select().distinct(&[
        session_step::entity::columns::SESSION,
        session_step::entity::columns::PR,
    ]);

    // `where_` replaces the previous condition, so bounds must be combined in a single `And`
    let mut conditions = Vec::new();
    if let Some(min_date) = min_date {
        conditions.push(Where::Gte(
            session_step::entity::columns::SESSION,
            min_date.into(),
        ));
    }
    if let Some(max_date) = max_date {
        conditions.push(Where::Lte(
            session_step::entity::columns::SESSION,
            max_date.into(),
        ));
    }
    if !conditions.is_empty() {
        select_builder = select_builder.where_(Where::And(conditions));
    }

    let mut flags: HashMap<u32, bool> = HashMap::new();
    for row in select_builder.fetch_in(conn)? {
        let session = row.get_u32(session_step::entity::columns::SESSION.as_ref());
        let pr = row.get_bool(session_step::entity::columns::PR.as_ref());
        *flags.entry(session).or_default() |= pr;
    }
    Ok(flags)
}

/// Returns full details for one session (series grouped by exercise, heart rate, GPS, speeds, device).
pub async fn get_session_details<R: Runtime>(
    app: &AppHandle<R>,
    settings: &SettingsLock,
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
/// File name for an exported session: its local date followed by its name.
pub fn session_file_name(timestamp: u32, name: &str) -> String {
    let date = Local
        .timestamp_opt(timestamp as i64, 0)
        .unwrap()
        .format("%Y-%m-%d");
    let name = name.trim().replace('/', "-");

    if name.is_empty() {
        format!("{date}.fit")
    } else {
        format!("{date} {name}.fit")
    }
}

/// Asks where to save the session as a FIT file and writes it there.
pub async fn download_session<R: Runtime>(
    app: &AppHandle<R>,
    settings: &SettingsLock,
    timestamp: u32,
) -> Result<(), String> {
    let (lang, weight_unit) = {
        let settings = settings.read().unwrap();
        (settings.language, settings.weight_unit)
    };

    // Without a name the file is just named after the date; a failure surfaces when writing the file
    let name = run_blocking(app, move |database| {
        SessionRepository::select_by_id(database, timestamp)
            .ok()
            .flatten()
            .map(|session| session.name)
    })
    .await
    .unwrap_or_default();
    let default_file = constants::HOME_DIR.join(session_file_name(timestamp, &name));
    let Some(file) = ask_save_path(
        app,
        &default_file,
        "FIT",
        "fit",
        &translate("select_file_save_fit", lang),
    )
    .await?
    else {
        return Ok(());
    };

    match write_session_fit(app, timestamp, file.clone(), lang, weight_unit).await {
        Ok(()) => {
            show_notification(NotificationDefinition {
                title: translate("ok_session_download", lang),
                body: translate_and_replace(
                    "export_file_path",
                    &[&file.display().to_string()],
                    lang,
                ),
                kind: NotificationKind::Temporal,
            });
            Ok(())
        }
        Err(e) => Err(report_error(
            e,
            lang,
            "error_session_download",
            "Error downloading session",
        )),
    }
}

/// Encodes the stored session as a FIT file at `path`.
pub async fn write_session_fit<R: Runtime>(
    app: &AppHandle<R>,
    timestamp: u32,
    path: PathBuf,
    lang: Languages,
    weight_unit: WeightUnit,
) -> Result<(), String> {
    run_blocking(app, move |database| {
        let session = database
            .run_in_connection(|conn| {
                let mut session = SessionRepository::select_by_id_in(conn, timestamp)?
                    .ok_or_else(|| format!("Session {timestamp} not found"))?;
                session.fetch_steps_relationship_in(conn)?;
                session.fetch_laps_relationship_in(conn)?;
                session.fetch_additional_data_relationship_in(conn)?;
                if session.workout.is_some() {
                    session.fetch_workout_obj_relationship_in(conn)?;
                }
                Ok(session)
            })
            .map_err(|e| e.to_string())?;

        FitWriter::from(&session)
            .write(path, lang, weight_unit)
            .map_err(|e| e.to_string())
    })
    .await
}

pub fn fetch_session_details(
    conn: &rusqlite_orm::rusqlite::Connection,
    timestamp: u32,
    with_add_data: bool,
    with_laps: bool,
) -> rusqlite_orm::errors::Result<SessionDetails> {
    Ok(
        fetch_sessions_details(conn, &[timestamp], with_add_data, with_laps)?
            .pop()
            .unwrap(),
    )
}

/// Details of every session in `timestamps`, in that order, with a fixed number of queries
/// whatever the number of sessions: each related table is read once for all of them.
///
/// # Panics
/// If any of the sessions doesn't exist.
pub fn fetch_sessions_details(
    conn: &rusqlite_orm::rusqlite::Connection,
    timestamps: &[u32],
    with_add_data: bool,
    with_laps: bool,
) -> rusqlite_orm::errors::Result<Vec<SessionDetails>> {
    if timestamps.is_empty() {
        return Ok(Vec::new());
    }
    let dates: Vec<Value> = timestamps.iter().map(|ts| (*ts).into()).collect();

    let mut sessions: HashMap<u32, Session> = SessionRepository::select()
        .where_(Where::In(session::entity::columns::DATE, dates.clone()))
        .fetch_in(conn)?
        .into_iter()
        .map(|s| (s.date, s))
        .collect();

    for step in SessionStepRepository::select()
        .where_(Where::In(entity::columns::SESSION, dates.clone()))
        .fetch_in(conn)?
    {
        if let Some(session) = sessions.get_mut(&step.session) {
            session.steps.push(step);
        }
    }

    let serials: HashSet<String> = sessions.values().filter_map(|s| s.device.clone()).collect();
    if !serials.is_empty() {
        let devices: HashMap<String, Device> = DeviceRepository::select()
            .where_(Where::In(
                device::entity::columns::SERIAL,
                serials.into_iter().map(|s| s.into()).collect(),
            ))
            .fetch_in(conn)?
            .into_iter()
            .map(|d| (d.serial.clone(), d))
            .collect();
        for session in sessions.values_mut() {
            session.device_obj = session
                .device
                .as_ref()
                .and_then(|serial| devices.get(serial).cloned());
        }
    }

    if with_add_data {
        for add_data in AdditionalDataRepository::select()
            .where_(Where::In(
                additional_data::entity::columns::SESSION,
                dates.clone(),
            ))
            .fetch_in(conn)?
        {
            if let Some(session) = sessions.get_mut(&add_data.session) {
                session.additional_data = Some(add_data);
            }
        }
    }

    if with_laps {
        for lap in LapRepository::select()
            .where_(Where::In(lap::entity::columns::SESSION, dates))
            .fetch_in(conn)?
        {
            if let Some(session) = sessions.get_mut(&lap.session) {
                session.laps.push(lap);
            }
        }
    }

    let keys: HashSet<(u16, u16)> = sessions
        .values()
        .flat_map(|s| &s.steps)
        .filter_map(|step| step.ex_cat.zip(step.ex_id))
        .collect();
    let mut exercises = if keys.is_empty() {
        Vec::new()
    } else {
        ExerciseRepository::select()
            .where_(Where::InMultiple(
                vec![
                    exercise::entity::columns::CATEGORY,
                    exercise::entity::columns::ID,
                ],
                keys.iter()
                    .map(|(cat, id)| vec![(*cat).into(), (*id).into()])
                    .collect(),
            ))
            .fetch_in(conn)?
    };
    fetch_exercises_muscles(conn, &mut exercises)?;
    let exercises: HashMap<(u16, u16), Exercise> = exercises
        .into_iter()
        .map(|e| ((e.category, e.id), e))
        .collect();

    for session in sessions.values_mut() {
        session.steps.sort_by_key(|s| s.idx);
        session.laps.sort_by_key(|l| l.idx);
    }

    Ok(timestamps
        .iter()
        .map(|ts| {
            let session = &sessions[ts];
            // Only this session's exercises: its muscles are computed over them
            let own_keys: HashSet<(u16, u16)> = session
                .steps
                .iter()
                .filter_map(|step| step.ex_cat.zip(step.ex_id))
                .collect();
            let own: Vec<Exercise> = own_keys
                .iter()
                .filter_map(|key| exercises.get(key).cloned())
                .collect();
            SessionDetails::from((
                session,
                own.as_slice(),
                session.steps.as_slice(),
                session.laps.as_slice(),
            ))
        })
        .collect())
}

/// Applies user edits (session notes and reps/weight/exercise of its series) and recomputes personal records.
pub async fn save_session_changes<R: Runtime>(
    app: &AppHandle<R>,
    settings: &SettingsLock,
    details: SessionStepsUpdate,
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

            SessionStepRepository::select()
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
                    // Rest steps have no exercise
                    if let Some(cat) = row.get_u16_nullable(entity::columns::EX_CAT.as_ref())
                        && let Some(id) = row.get_u16_nullable(entity::columns::EX_ID.as_ref())
                    {
                        exercises.insert((cat, id));
                    }
                });
            for serie in &details.sets {
                if let Some(cat) = serie.ex_cat
                    && let Some(id) = serie.ex_id
                {
                    exercises.insert((cat, id));
                }
            }

            let has_suggestion = |tx: &rusqlite_orm::rusqlite::Transaction,
                                  (cat, id): (u16, u16)| {
                fetch_progression_suggestion(tx, cat, id)
                    .ok()
                    .flatten()
                    .is_some()
            };
            let suggested_before: HashSet<(u16, u16)> = exercises
                .iter()
                .copied()
                .filter(|ex| has_suggestion(tx, *ex))
                .collect();

            for serie in &details.sets {
                SessionStepRepository::update()
                    .set(entity::columns::REPS, serie.reps.into())
                    .set(entity::columns::WEIGHT, serie.weight.into())
                    .set(entity::columns::EX_CAT, serie.ex_cat.into())
                    .set(entity::columns::EX_ID, serie.ex_id.into())
                    .where_(Where::And(vec![
                        Where::Eq(entity::columns::SESSION, details.timestamp.into()),
                        Where::Eq(entity::columns::IDX, serie.idx.into()),
                    ]))
                    .execute_in(tx)?;
            }

            update_prs(tx, &exercises, &[details.timestamp], Some(lang))?;

            // Evaluated once: tells both which exercises now progress and whether any does
            let suggested_after = progression_suggestions(tx)?;
            let newly_suggested = exercises
                .iter()
                .any(|ex| !suggested_before.contains(ex) && suggested_after.contains(ex));
            Ok((newly_suggested, !suggested_after.is_empty()))
        })
    })
    .await;

    match res {
        Ok((newly_suggested, any_progression)) => {
            info!("Session updated succesfully");
            show_notification(NotificationDefinition {
                title: translate("ok_update_session", lang),
                body: "".to_string(),
                kind: NotificationKind::Temporal,
            });

            if newly_suggested {
                show_notification(NotificationDefinition {
                    title: translate("progression_available", lang),
                    body: translate("progression_available_msg", lang),
                    kind: NotificationKind::Temporal,
                });
            }
            let payload: bool = any_progression;
            let _ = app.emit("progressions_available", payload);

            Ok(())
        }
        Err(e) => Err(report_error(
            e,
            lang,
            "error_update_session",
            "Error updating session",
        )),
    }
}

/// Asks the user for `.fit` files and imports them; returns the number of sessions imported.
pub async fn import_from_files<R: Runtime>(
    app: &AppHandle<R>,
    settings: &SettingsLock,
) -> Result<usize, String> {
    let lang = { settings.read().unwrap().language };
    let files = ask_open_paths(app, "FIT", "fit", &translate("select_files_import", lang)).await?;

    if !files.is_empty() {
        let app = app.clone();
        let res = tokio::task::spawn_blocking(move || _import_from_files(&app, files.as_slice()))
            .await
            .map_err(|e| e.to_string())
            .flatten();
        return res;
    }

    Ok(0)
}

pub fn _import_from_files<R: Runtime>(
    app: &AppHandle<R>,
    files: &[PathBuf],
) -> Result<usize, String> {
    let lang = app.state::<SettingsLock>().read().unwrap().language;
    let files: Vec<FitFile> = files
        .iter()
        .filter_map(|path| match FitFile::read(path) {
            Ok(file) => Some(file),
            Err(e) => {
                error!("Error reading session file {}: {}", path.display(), e);
                notify_parse_error(&path.display().to_string(), &e.to_string(), lang);
                None
            }
        })
        .collect();
    let database = app.state::<DatabasePool>();
    let res = database
        .run_in_transaction(|tx| {
            let imported = import_file_list(tx, &files, lang)?;
            Ok((imported.sessions, imported.any_progression))
        })
        .map_err(|e| e.to_string());

    match res {
        Ok((res, any_progression)) => {
            if let Some(any_progression) = any_progression {
                let payload: bool = any_progression;
                let _ = app.emit("progressions_available", payload);
            }
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

/// Imports as sessions of `device` the `.fit` files fetched from it. Those that fail to parse, or
/// all of them if the transaction is rolled back, are stored as pending. With `sync_mark`, the
/// device's last activity sync is moved to it in the same transaction.
pub fn import_pending_sessions<R: Runtime>(
    app: &AppHandle<R>,
    db: &DatabasePool,
    files: &[FitFile],
    device: &mut Device,
    lang: Languages,
    sync_mark: Option<u32>,
) -> Result<Vec<u32>, DatabaseError> {
    let res = db.run_in_transaction(|tx| {
        let res = if !files.is_empty() {
            info!("Fetched {} activity files", files.len());
            import_file_list(tx, files, lang)?
        } else {
            ImportedFiles::default()
        };

        if let Some(mark) = sync_mark {
            device.last_activity_sync = Some(mark);
            device.update_by_id_in(tx)?;
        }

        Ok(res)
    });

    let pending_dir = &constants::PENDING_DOWNLOAD_SESSIONS_FOLDER;
    match &res {
        Ok(imported) => {
            if let Some(any_progression) = imported.any_progression {
                let payload: bool = any_progression;
                let _ = app.emit("progressions_available", payload);
            }
            store_pending_files(pending_dir, &device.serial, imported.failed.iter().copied());
        }
        Err(_) => store_pending_files(pending_dir, &device.serial, files),
    }

    res.map(|imported| imported.sessions)
}

/// Outcome of `import_file_list`.
#[derive(Default)]
pub struct ImportedFiles<'a> {
    /// Dates of the sessions inserted.
    pub sessions: Vec<u32>,
    /// Files that could not be parsed.
    pub failed: Vec<&'a FitFile>,
    /// Whether any exercise has a progression suggestion pending, checked only when sessions
    /// were inserted (otherwise it cannot have changed).
    pub any_progression: Option<bool>,
}

/// Shows the notification of a session file that could not be read or parsed.
fn notify_parse_error(name: &str, error: &str, lang: Languages) {
    let title = Path::new(name)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| name.to_string());
    show_notification(NotificationDefinition {
        title,
        body: translate_and_replace("error_parsing_session", &[error], lang),
        kind: NotificationKind::Persistant,
    });
}

/// Parses a batch of `.FIT` files in parallel and inserts each new session (plus its exercises/series/heart rate/GPS/speeds) in the given transaction, then refreshes personal records.
pub fn import_file_list<'a>(
    tx: &mut rusqlite_orm::rusqlite::Transaction,
    files: &'a [FitFile],
    lang: Languages,
) -> Result<ImportedFiles<'a>, DatabaseError> {
    let mut success = Vec::new();
    let mut handled_exercises: HashSet<(u16, u16)> = HashSet::new();

    let (mut sessions, failed): (Vec<_>, Vec<_>) = files.par_iter().partition_map(|file| {
        info!("Parsing session file {}", file.name);
        match Session::try_from(FitParser::from_fit_file(file.clone())) {
            Ok(session) => Either::Left(session),
            Err(e) => {
                error!("Error parsing session file {}: {}", file.name, e);
                notify_parse_error(&file.name, &e.to_string(), lang);
                Either::Right(file)
            }
        }
    });

    sessions.sort_by_key(|s| s.date);

    let mut existing_sessions = if sessions.is_empty() {
        HashSet::new()
    } else {
        SessionRepository::select()
            .columns(&[session::entity::columns::DATE])
            .where_(Where::In(
                session::entity::columns::DATE,
                sessions.iter().map(|s| s.date.into()).collect(),
            ))
            .fetch_in(tx)?
            .iter()
            .map(|row| row.get_u32(session::entity::columns::DATE.as_ref()))
            .collect()
    };
    let known_devices: HashSet<_> = if sessions.is_empty() {
        HashSet::new()
    } else {
        DeviceRepository::select()
            .columns(&[device::entity::columns::SERIAL])
            .fetch_in(tx)?
            .iter()
            .map(|row| row.get_text(device::entity::columns::SERIAL.as_ref()))
            .collect()
    };
    let mut handled_workouts = HashSet::new();

    for session in &mut sessions {
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
                // The serial recorded in the file may belong to a device that isn't registered
                session.device = session.device.take().filter(|s| known_devices.contains(s));
                insert_session(tx, session, &mut handled_workouts)?;

                for serie in &session.steps {
                    if let Some(cat) = serie.ex_cat
                        && let Some(id) = serie.ex_id
                    {
                        handled_exercises.insert((cat, id));
                    }
                }

                let date = session.date;
                existing_sessions.insert(date);
                success.push(date);

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

    let mut any_progression = None;
    if !success.is_empty() {
        show_notification(NotificationDefinition {
            title: translate("ok_import_sessions", lang),
            body: translate_and_replace("imported_n_sessions", &[&success.len().to_string()], lang),
            kind: NotificationKind::Temporal,
        });
        update_prs(tx, &handled_exercises, &success, Some(lang))?;

        // Evaluated once: tells both whether the imported exercises progress and whether any does
        let suggested = progression_suggestions(tx)?;
        if handled_exercises.iter().any(|e| suggested.contains(e)) {
            show_notification(NotificationDefinition {
                title: translate("progression_available", lang),
                body: translate("progression_available_msg", lang),
                kind: NotificationKind::Temporal,
            });
        }

        any_progression = Some(!suggested.is_empty());
    }

    Ok(ImportedFiles {
        sessions: success,
        failed,
        any_progression,
    })
}

/// Stores the notes of a single session, creating its additional data row if needed. Blank notes are stored as `NULL`.
/// Inserts a session with its sets, laps and additional data, creating its
/// workout (and steps, if missing) when the session references one.
/// Child collections are restored on the session after insertion.
pub fn insert_session(
    tx: &mut rusqlite_orm::rusqlite::Transaction,
    session: &mut Session,
    handled_workouts: &mut HashSet<String>,
) -> Result<(), DatabaseError> {
    let mut series = std::mem::take(&mut session.steps);
    let mut laps = std::mem::take(&mut session.laps);
    let mut add_data = session.additional_data.take();

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
                    connect_id: None,
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

    SessionRepository::insert().item(session).execute_in(tx)?;

    if !series.is_empty() {
        let mut insert = SessionStepRepository::insert();
        for serie in &mut series {
            insert = insert.item(serie);
        }
        insert.execute_in(tx)?;
    }

    if let Some(additional_data) = &mut add_data {
        AdditionalDataRepository::insert()
            .item(additional_data)
            .execute_in(tx)?;
    }

    if !laps.is_empty() {
        let mut insert = LapRepository::insert();
        for lap in &mut laps {
            insert = insert.item(lap)
        }
        insert.execute_in(tx)?;
    }

    session.steps = series;
    session.laps = laps;
    session.additional_data = add_data;

    Ok(())
}

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
pub fn update_pending_geolocation<R: Runtime>(app: &AppHandle<R>, db: &DatabasePool) {
    let _lock = GELOCATION_MUTEX.lock().unwrap();
    info!("Looking for pending geocode workouts...");
    let pending = {
        let unnamed_sessions = SessionRepository::select()
            .columns(&[session::entity::columns::DATE])
            .where_(Where::Eq(session::entity::columns::NAME, "".into()))
            .to_subquery();

        AdditionalDataRepository::select()
            .columns(&[
                additional_data::entity::columns::SESSION,
                additional_data::entity::columns::COORDINATES,
            ])
            .where_(Where::And(vec![
                Where::InSub(additional_data::entity::columns::SESSION, unnamed_sessions),
                Where::NotNull(additional_data::entity::columns::COORDINATES),
            ]))
            .order_by(OrderBy::Desc(additional_data::entity::columns::SESSION))
            .fetch(db)
            .map(|rows| {
                rows.into_iter()
                    .map(|row| {
                        let mut add_data = AdditionalData::new(
                            row.get_u32(additional_data::entity::columns::SESSION.as_ref()),
                        );
                        add_data.coordinates = row.get_blob_nullable(
                            additional_data::entity::columns::COORDINATES.as_ref(),
                        );
                        add_data
                    })
                    .collect::<Vec<_>>()
            })
    };
    match pending {
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

pub async fn get_heatmap_data<R: Runtime>(
    app: &AppHandle<R>,
) -> Result<Vec<Vec<(u32, u16)>>, String> {
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
            let prs = SessionStepRepository::select()
                .columns(&[session_step::entity::columns::SESSION])
                .where_(Where::And(vec![
                    Where::Eq(session_step::entity::columns::PR, true.into()),
                    Where::Gte(session_step::entity::columns::SESSION, ts.into()),
                ]))
                .fetch_in(conn)?;

            let mut pr_counts: HashMap<u32, u16> = HashMap::new();
            for pr in &prs {
                *pr_counts
                    .entry(pr.get_u32(session_step::entity::columns::SESSION.as_ref()))
                    .or_default() += 1;
            }

            let mut res = vec![vec![(0_u32, 0_u16); 31]; 12];
            sessions.iter().for_each(|session| {
                let date = session.get_u32(session::entity::columns::DATE.as_ref());
                let training_load =
                    session.get_u32(session::entity::columns::TRAINING_LOAD.as_ref());
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

#[allow(dead_code)]
pub fn merge_sessions(
    database: &DatabasePool,
    to_date: u32,
    from_date: u32,
) -> Result<(), DatabaseError> {
    let mut to = SessionRepository::select_by_id(database, to_date)
        .expect("Error on database query")
        .unwrap_or_else(|| panic!("Could not find session with timestamp {}", to_date));
    to.fetch_additional_data_relationship(database)
        .expect("Error on database query");
    to.fetch_laps_relationship(database)
        .expect("Error on database query");
    to.fetch_steps_relationship(database)
        .expect("Error on database query");

    let mut from = SessionRepository::select_by_id(database, from_date)
        .expect("Error on database query")
        .unwrap_or_else(|| panic!("Could not find session with timestamp {}", from_date));
    from.fetch_additional_data_relationship(database)
        .expect("Error on database query");
    from.fetch_laps_relationship(database)
        .expect("Error on database query");
    from.fetch_steps_relationship(database)
        .expect("Error on database query");

    to.total_elapsed_time += from.total_elapsed_time;
    to.active_time += from.active_time;
    to.training_load += from.training_load;
    to.active_calories += from.active_calories;

    match &mut to.additional_data {
        Some(add_data) => {
            if let Some(add_data2) = &mut from.additional_data {
                fn merge_vec<T>(to: &mut Option<Vec<T>>, from: &mut Option<Vec<T>>) {
                    if let Some(v2) = from.take() {
                        match to {
                            Some(v) => v.extend(v2),
                            None => *to = Some(v2),
                        }
                    }
                }

                merge_vec(&mut add_data.heart_rates, &mut add_data2.heart_rates);
                merge_vec(&mut add_data.coordinates, &mut add_data2.coordinates);
                merge_vec(&mut add_data.speeds, &mut add_data2.speeds);

                add_data.distance = match (add_data.distance, add_data2.distance) {
                    (Some(d1), Some(d2)) => Some(d1 + d2),
                    (d1, d2) => d1.or(d2),
                };

                add_data.max_hr = match (add_data.max_hr, add_data2.max_hr) {
                    (Some(hr1), Some(hr2)) => Some(hr1.max(hr2)),
                    (hr1, hr2) => hr1.or(hr2),
                };

                if let Some(notes2) = add_data2.notes.take() {
                    match &mut add_data.notes {
                        Some(notes) => {
                            notes.push(' ');
                            notes.push_str(&notes2);
                        }
                        None => add_data.notes = Some(notes2),
                    }
                }
            }
        }
        None => {
            to.additional_data = from.additional_data.take().map(|mut add_data| {
                add_data.session = to.date;
                add_data
            })
        }
    }

    for lap in &from.laps {
        let mut lap = lap.clone();
        lap.session = to.date;
        lap.idx = to.laps.len() as u16;
        to.laps.push(lap);
    }

    for set in &from.steps {
        let mut set = set.clone();
        set.session = to.date;
        set.idx = to.steps.len() as u8;
        to.steps.push(set);
    }

    database.run_in_transaction(|tx| {
        to.delete_by_id_in(tx)?;
        from.delete_by_id_in(tx)?;

        insert_session(tx, &mut to, &mut HashSet::new())?;

        let handled_exercises = to
            .steps
            .iter()
            .filter_map(|s| {
                if let Some(cat) = s.ex_cat
                    && let Some(id) = s.ex_id
                {
                    Some((cat, id))
                } else {
                    None
                }
            })
            .collect();
        update_prs(tx, &handled_exercises, &[to.date], None)?;

        Ok(())
    })
}

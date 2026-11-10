use std::collections::{HashMap, HashSet};

use chrono::{Datelike, Days, Local, Months, TimeZone, offset::LocalResult};
use rayon::{iter::Either, prelude::*};
use rusqlite_orm::{
    dao::Repository,
    database::DatabasePool,
    errors::DatabaseError,
    types::{order_by::OrderBy, value::Value, where_clause::Where},
};
use tauri_plugin_log::log::{error, info, warn};

use super::helpers::notify_parse_error;
use crate::{
    dao::{
        device::{self, Device, DeviceRepository},
        exercise::{self, Exercise, ExerciseRepository},
        session::{self, Session, SessionRepository},
        session_data::{self, SessionData, SessionDataRepository},
        session_lap::{self, SessionLapRepository},
        session_step::{self, SessionStepRepository, entity},
        workout::{self, Workout, WorkoutRepository},
        workout_step::WorkoutStepRepository,
    },
    dto::{
        notifications::{NotificationDefinition, NotificationKind},
        sessions::{SessionDetails, SessionListItem, SessionStepsUpdate},
    },
    fit::{FitFile, parser::FitParser},
    logic::{
        exercises::queries::{
            fetch_exercises_muscles, fetch_progression_suggestion, progression_suggestions,
            update_prs,
        },
        notifications::show_notification,
    },
    utils::translations::{Languages, translate, translate_and_replace},
};

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
        for add_data in SessionDataRepository::select()
            .where_(Where::In(
                session_data::entity::columns::SESSION,
                dates.clone(),
            ))
            .fetch_in(conn)?
        {
            if let Some(session) = sessions.get_mut(&add_data.session) {
                session.data = Some(add_data);
            }
        }
    }

    if with_laps {
        for lap in SessionLapRepository::select()
            .where_(Where::In(session_lap::entity::columns::SESSION, dates))
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
            Ok(mut session) => {
                // Known up front when downloaded from Connect, saving the later lookup
                if let Some(id) = file.activity_id {
                    session.connect_id = Some(id.to_string());
                }
                Either::Left(session)
            }
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
    let mut add_data = session.data.take();

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
        SessionDataRepository::insert()
            .item(additional_data)
            .execute_in(tx)?;
    }

    if !laps.is_empty() {
        let mut insert = SessionLapRepository::insert();
        for lap in &mut laps {
            insert = insert.item(lap)
        }
        insert.execute_in(tx)?;
    }

    session.steps = series;
    session.laps = laps;
    session.data = add_data;

    Ok(())
}

pub fn update_session_notes(
    tx: &rusqlite_orm::rusqlite::Transaction,
    session: u32,
    notes: Option<&str>,
) -> rusqlite_orm::errors::Result<()> {
    let notes = notes.filter(|n| !n.trim().is_empty()).map(str::to_string);

    let updated = SessionDataRepository::update()
        .set(session_data::entity::columns::NOTES, notes.clone().into())
        .where_(Where::Eq(
            session_data::entity::columns::SESSION,
            session.into(),
        ))
        .execute_in(tx)?;

    if updated == 0 && notes.is_some() {
        let mut add_data = SessionData::new(session);
        add_data.notes = notes;
        SessionDataRepository::insert()
            .item(&mut add_data)
            .execute_in(tx)?;
    }

    Ok(())
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
    to.fetch_data_relationship(database)
        .expect("Error on database query");
    to.fetch_laps_relationship(database)
        .expect("Error on database query");
    to.fetch_steps_relationship(database)
        .expect("Error on database query");

    let mut from = SessionRepository::select_by_id(database, from_date)
        .expect("Error on database query")
        .unwrap_or_else(|| panic!("Could not find session with timestamp {}", from_date));
    from.fetch_data_relationship(database)
        .expect("Error on database query");
    from.fetch_laps_relationship(database)
        .expect("Error on database query");
    from.fetch_steps_relationship(database)
        .expect("Error on database query");

    to.total_elapsed_time += from.total_elapsed_time;
    to.active_time += from.active_time;
    to.training_load += from.training_load;
    to.active_calories += from.active_calories;

    match &mut to.data {
        Some(add_data) => {
            if let Some(add_data2) = &mut from.data {
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
            to.data = from.data.take().map(|mut add_data| {
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

/// Stores the edited notes and sets of a session and recomputes the records of the exercises
/// involved. Returns whether some exercise got a new progression suggestion and whether any
/// exercise has one.
pub fn apply_session_changes(
    tx: &rusqlite_orm::rusqlite::Transaction,
    details: &SessionStepsUpdate,
    lang: Languages,
) -> rusqlite_orm::errors::Result<(bool, bool)> {
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

    let has_suggestion = |tx: &rusqlite_orm::rusqlite::Transaction, (cat, id): (u16, u16)| {
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
}

/// Name of the session, `None` if it doesn't exist.
pub fn session_name(
    database: &DatabasePool,
    timestamp: u32,
) -> rusqlite_orm::errors::Result<Option<String>> {
    Ok(SessionRepository::select_by_id(database, timestamp)?.map(|session| session.name))
}

/// Links the session to its Garmin Connect activity.
pub fn set_session_connect_id(
    database: &DatabasePool,
    timestamp: u32,
    id: u64,
) -> rusqlite_orm::errors::Result<usize> {
    SessionRepository::update()
        .where_(Where::Eq(session::entity::columns::DATE, timestamp.into()))
        .set(session::entity::columns::CONNECT_ID, id.into())
        .execute(database)
}

/// The session with its sets, laps, additional data and workout, `None` if it doesn't exist.
pub fn full_session(
    conn: &rusqlite_orm::rusqlite::Connection,
    timestamp: u32,
) -> rusqlite_orm::errors::Result<Option<Session>> {
    let Some(mut session) = SessionRepository::select_by_id_in(conn, timestamp)? else {
        return Ok(None);
    };
    session.fetch_steps_relationship_in(conn)?;
    session.fetch_laps_relationship_in(conn)?;
    session.fetch_data_relationship_in(conn)?;
    if session.workout.is_some() {
        session.fetch_workout_obj_relationship_in(conn)?;
    }
    Ok(Some(session))
}

/// Coordinates of the unnamed sessions recorded with GPS, newest first, to name them after their
/// location.
pub fn pending_geolocation(db: &DatabasePool) -> rusqlite_orm::errors::Result<Vec<SessionData>> {
    let unnamed_sessions = SessionRepository::select()
        .columns(&[session::entity::columns::DATE])
        .where_(Where::Eq(session::entity::columns::NAME, "".into()))
        .to_subquery();

    SessionDataRepository::select()
        .columns(&[
            session_data::entity::columns::SESSION,
            session_data::entity::columns::COORDINATES,
        ])
        .where_(Where::And(vec![
            Where::InSub(session_data::entity::columns::SESSION, unnamed_sessions),
            Where::NotNull(session_data::entity::columns::COORDINATES),
        ]))
        .order_by(OrderBy::Desc(session_data::entity::columns::SESSION))
        .fetch(db)
        .map(|rows| {
            rows.into_iter()
                .map(|row| {
                    let mut add_data = SessionData::new(
                        row.get_u32(session_data::entity::columns::SESSION.as_ref()),
                    );
                    add_data.coordinates =
                        row.get_blob_nullable(session_data::entity::columns::COORDINATES.as_ref());
                    add_data
                })
                .collect()
        })
}

/// Renames the session; returns the number of rows changed.
pub fn set_session_name(
    db: &DatabasePool,
    session: u32,
    name: &str,
) -> rusqlite_orm::errors::Result<usize> {
    SessionRepository::update()
        .set(session::entity::columns::NAME, name.into())
        .where_(Where::Eq(session::entity::columns::DATE, session.into()))
        .execute(db)
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

/// Imports the `.fit` files fetched from `device` and, with `sync_mark`, moves its last activity
/// sync to it.
pub fn import_device_files<'a>(
    tx: &mut rusqlite_orm::rusqlite::Transaction,
    files: &'a [FitFile],
    device: &mut Device,
    lang: Languages,
    sync_mark: Option<u32>,
) -> Result<ImportedFiles<'a>, DatabaseError> {
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
}

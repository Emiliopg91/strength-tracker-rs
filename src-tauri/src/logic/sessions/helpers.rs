use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use chrono::{Datelike, Days, Local, Months, TimeZone, offset::LocalResult};
use rayon::{iter::Either, prelude::*};
use rusqlite_orm::{
    database::DatabasePool,
    errors::DatabaseError,
    rusqlite::{Connection, Transaction},
    types::value::Value,
};
use tauri::{AppHandle, Emitter, Manager, Runtime};
use tauri_plugin_log::log::{error, info, warn};
use tokio::sync::Mutex;

use crate::{
    ConnectLock, SettingsLock,
    dao::{
        device::Device, exercise::Exercise, session::Session, session_data::SessionData,
        session_lap::SessionLap, session_step::SessionStep, settings::WeightUnit,
    },
    dto::{
        notifications::{NotificationDefinition, NotificationKind},
        sessions::{SessionDetails, SessionListItem, SessionLocation, SessionStepsUpdate},
    },
    fit::{FitFile, parser::FitParser, writer::FitWriter},
    logic::{
        exercises::helpers::{progression_suggestions, progression_suggestions_among},
        language,
        notifications::show_notification,
        pool, report_error, store_pending_files,
    },
    utils::{
        constants,
        translations::{Languages, translate, translate_and_replace},
    },
};

pub(super) async fn retrieve_session_connect_id<R: Runtime>(app: &AppHandle<R>, timestamp: u32) {
    let connect = app.state::<ConnectLock>();
    let mut connect = connect.lock().await;
    if connect.logged_user().is_some()
        && let Ok(Some(id)) = connect.find_session_activity(timestamp).await
    {
        let _ = pool(app)
            .run_in_connection(async move |conn| {
                Ok(Session::set_connect_id(conn, timestamp, id).await?)
            })
            .await;
    }
}

/// File name for an exported session: its local date followed by its name.
pub(crate) fn session_file_name(timestamp: u32, name: &str) -> String {
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

/// Encodes the stored session as a FIT file at `path`.
pub(crate) async fn write_session_fit<R: Runtime>(
    app: &AppHandle<R>,
    timestamp: u32,
    path: PathBuf,
    lang: Languages,
    weight_unit: WeightUnit,
) -> Result<(), String> {
    let session = pool(app)
        .run_in_connection(async move |conn| Ok(Session::get_full(conn, timestamp).await?))
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("Session {timestamp} not found"))?;

    // Encoding and writing the file is blocking work too
    tokio::task::spawn_blocking(move || {
        FitWriter::from(&session)
            .write(path, lang, weight_unit)
            .map_err(|e| e.to_string())
    })
    .await
    .expect("FIT writing task panicked")
}

pub(crate) async fn import_file_paths<R: Runtime>(
    app: &AppHandle<R>,
    files: Vec<PathBuf>,
) -> Result<usize, String> {
    let lang = language(app);
    // Reading the files is blocking work
    let files: Vec<FitFile> = tokio::task::spawn_blocking(move || {
        files
            .iter()
            .filter_map(|path| match FitFile::read(path) {
                Ok(file) => Some(file),
                Err(e) => {
                    error!("Error reading session file {}: {}", path.display(), e);
                    notify_parse_error(&path.display().to_string(), &e.to_string(), lang);
                    None
                }
            })
            .collect()
    })
    .await
    .expect("FIT reading task panicked");

    let res = pool(app)
        .run_in_transaction(async move |tx| Ok(import_file_list(tx, &files).await?))
        .await
        .map_err(|e| e.to_string());

    finish_session_import(
        app,
        res.map(|imported| {
            notify_import(app, &imported, lang);
            imported.sessions
        }),
    )
}

/// Imports as sessions of `device` the `.fit` files fetched from it. Those that fail to parse, or
/// all of them if the transaction is rolled back, are stored as pending. With `sync_mark`, the
/// device's last activity sync is moved to it in the same transaction.
pub(crate) async fn import_pending_sessions<R: Runtime>(
    app: &AppHandle<R>,
    files: Vec<FitFile>,
    device: &mut Device,
    lang: Languages,
    sync_mark: Option<u32>,
) -> Result<Vec<u32>, DatabaseError> {
    // Shared with the transaction, as they are still needed to store them as pending if it fails
    let files = Arc::new(files);
    let tx_files = Arc::clone(&files);
    let mut tx_device = device.clone();
    let res = pool(app)
        .run_in_transaction(async move |tx| {
            let imported = import_device_files(tx, &tx_files, &mut tx_device, sync_mark).await?;
            Ok((imported, tx_device))
        })
        .await;

    let pending_dir = &constants::PENDING_DOWNLOAD_SESSIONS_FOLDER;
    match res {
        Ok((imported, updated_device)) => {
            *device = updated_device;
            notify_import(app, &imported, lang);
            store_pending_files(
                pending_dir,
                &device.serial,
                &imported
                    .failed
                    .iter()
                    .map(|(file, _)| file)
                    .collect::<Vec<_>>(),
            )
            .await;
            Ok(imported.sessions)
        }
        Err(e) => {
            store_pending_files(
                pending_dir,
                &device.serial,
                &files.iter().collect::<Vec<_>>(),
            )
            .await;
            Err(e)
        }
    }
}

/// Ends an import of sessions: looks up the place names of the imported ones in the background
/// and returns how many there are, or notifies the error.
pub(crate) fn finish_session_import<R: Runtime, E: std::fmt::Display>(
    app: &AppHandle<R>,
    res: Result<Vec<u32>, E>,
) -> Result<usize, String> {
    match res {
        Ok(sessions) => {
            if !sessions.is_empty() {
                spawn_geolocation_update(app);
            }
            Ok(sessions.len())
        }
        Err(e) => Err(report_error(
            e,
            &app.state::<SettingsLock>(),
            "error_import_sessions",
            "Error importing sessions",
        )),
    }
}

/// Looks up the place names of the sessions pending geolocation in the background.
pub(crate) fn spawn_geolocation_update<R: Runtime>(app: &AppHandle<R>) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move { update_pending_geolocation(&app).await });
}

/// Tells the user, once the import is committed, how it went: files that failed to parse,
/// sessions imported, new records and progressions. Also lets the frontend know whether any
/// progression is pending.
fn notify_import<R: Runtime>(app: &AppHandle<R>, imported: &ImportedFiles, lang: Languages) {
    for (file, e) in &imported.failed {
        notify_parse_error(&file.name, e, lang);
    }

    if !imported.sessions.is_empty() {
        show_notification(NotificationDefinition {
            title: translate("ok_import_sessions", lang),
            body: translate_and_replace(
                "imported_n_sessions",
                &[&imported.sessions.len().to_string()],
                lang,
            ),
            kind: NotificationKind::Temporal,
        });
    }
    if imported.new_prs {
        notify_new_prs(lang);
    }
    if imported.imported_progression {
        notify_progression_available(lang);
    }

    if let Some(any_progression) = imported.any_progression {
        // Typed binding: the TypeScript generator reads the payload type from it
        let payload: bool = any_progression;
        let _ = app.emit("progressions_available", payload);
    }
}

/// Congratulates the user on a new personal record.
pub(super) fn notify_new_prs(lang: Languages) {
    show_notification(NotificationDefinition {
        title: translate("new_record", lang),
        body: translate("contratulations_pr", lang),
        kind: NotificationKind::Temporal,
    });
}

/// Tells the user some exercise is ready to progress.
pub(super) fn notify_progression_available(lang: Languages) {
    show_notification(NotificationDefinition {
        title: translate("progression_available", lang),
        body: translate("progression_available_msg", lang),
        kind: NotificationKind::Temporal,
    });
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

pub(super) static GELOCATION_MUTEX: Mutex<()> = Mutex::const_new(());

/// Names the unnamed sessions recorded with GPS after the place where they started.
pub(crate) async fn update_pending_geolocation<R: Runtime>(app: &AppHandle<R>) {
    let _lock = GELOCATION_MUTEX.lock().await;
    info!("Looking for sessions pending geolocation...");
    let pending = match pool(app)
        .run_in_connection(async |conn| Ok(SessionData::get_pending_geolocation(conn).await?))
        .await
    {
        Ok(pending) => pending,
        Err(e) => {
            error!("Error while looking for pending sessions: {}", e);
            return;
        }
    };

    info!("Found {} pending sessions", pending.len());
    for data in pending {
        let session = data.session;
        // The blob may hold only invalid positions
        let Some((lat, lon)) = data
            .get_coordinates_degrees()
            .and_then(|coords| coords.into_iter().flatten().next())
        else {
            warn!("Session @ {session} has no valid position to geolocate");
            continue;
        };

        match geocode(lat, lon).await {
            Ok(Some(location)) => {
                let name = location.clone();
                let res = pool(app)
                    .run_in_connection(async move |conn| {
                        Ok(Session::set_name(conn, session, &name).await?)
                    })
                    .await;
                match res {
                    Ok(1) => {
                        info!("Updated location for session @ {session}");
                        let payload: SessionLocation = SessionLocation { session, location };
                        let _ = app.emit("session_location_update", payload);
                    }
                    Ok(_) => info!("Missing session @ {session}"),
                    Err(e) => error!("Error while updating session {session}: {e}"),
                }
            }
            Ok(None) => {}
            Err(e) => error!("Error geolocating session @ {session}: {e}"),
        }

        // Nominatim allows one request per second
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}

/// Name of the place at `lat`, `lon` from Nominatim: the most specific of village, town, city,
/// state and country it gives, `None` if it gives none.
async fn geocode(lat: f64, lon: f64) -> Result<Option<String>, String> {
    let url =
        format!("https://nominatim.openstreetmap.org/reverse?format=json&lat={lat}&lon={lon}");
    let response = reqwest::Client::new()
        .get(&url)
        .header(reqwest::header::USER_AGENT, "strength-coach-for-garmin")
        .header(reqwest::header::ACCEPT, "application/json")
        .send()
        .await
        .map_err(|e| format!("Error on geocode query: {e}"))?;
    if response.status() != reqwest::StatusCode::OK {
        return Err(format!("Invalid status code: {}", response.status()));
    }

    let json: serde_json::Value = response
        .json()
        .await
        .map_err(|e| format!("Error parsing response: {e}"))?;
    let Some(address) = json.get("address") else {
        return Ok(None);
    };
    Ok(["village", "town", "city", "state", "country"]
        .iter()
        .find_map(|key| address.get(key))
        .map(|value| value.to_string().replace('"', "")))
}

/// Lists sessions newest first (within `[from, to]` and of the sport given, if any), flagged
/// with whether they have sets and records.
pub(super) async fn fetch_sessions(
    conn: &Connection,
    from: Option<i32>,
    to: Option<i32>,
    sport: Option<u8>,
    sub_sport: Option<u8>,
) -> rusqlite_orm::errors::Result<Vec<SessionListItem>> {
    let sessions = Session::get_sessions(
        conn,
        from.map(i64::from),
        to.map(i64::from),
        sport,
        sub_sport,
    )
    .await?;
    let set_flags = SessionStep::get_set_flags(conn, from, to).await?;

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

/// Details of the session.
///
/// # Panics
/// If the session doesn't exist.
pub(crate) async fn fetch_session_details(
    conn: &Connection,
    timestamp: u32,
    with_add_data: bool,
    with_laps: bool,
) -> rusqlite_orm::errors::Result<SessionDetails> {
    Ok(
        fetch_sessions_details(conn, &[timestamp], with_add_data, with_laps)
            .await?
            .pop()
            .unwrap(),
    )
}

/// Details of every session in `timestamps`, in that order, with a fixed number of queries
/// whatever the number of sessions: each related table is read once for all of them.
///
/// # Panics
/// If any of the sessions doesn't exist.
pub(crate) async fn fetch_sessions_details(
    conn: &Connection,
    timestamps: &[u32],
    with_add_data: bool,
    with_laps: bool,
) -> rusqlite_orm::errors::Result<Vec<SessionDetails>> {
    if timestamps.is_empty() {
        return Ok(Vec::new());
    }
    let dates: Vec<Value> = timestamps.iter().map(|ts| (*ts).into()).collect();

    let mut sessions: HashMap<u32, Session> = Session::get_by_dates(conn, &dates)
        .await?
        .into_iter()
        .map(|s| (s.date, s))
        .collect();

    for step in SessionStep::get_by_sessions(conn, &dates).await? {
        if let Some(session) = sessions.get_mut(&step.session) {
            session.steps.push(step);
        }
    }

    let serials: HashSet<String> = sessions.values().filter_map(|s| s.device.clone()).collect();
    if !serials.is_empty() {
        let devices: HashMap<String, Device> = Device::get_by_serials(conn, &serials)
            .await?
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
        for add_data in SessionData::get_by_sessions(conn, &dates).await? {
            if let Some(session) = sessions.get_mut(&add_data.session) {
                session.data = Some(add_data);
            }
        }
    }

    if with_laps {
        for lap in SessionLap::get_by_sessions(conn, &dates).await? {
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
    let mut exercises = Exercise::get_by_keys(conn, &keys).await?;
    Exercise::fill_muscles(conn, &mut exercises).await?;
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

/// Outcome of `import_file_list`.
#[derive(Default)]
pub(crate) struct ImportedFiles {
    /// Dates of the sessions inserted.
    pub sessions: Vec<u32>,
    /// Files that could not be parsed, with the parse error.
    pub failed: Vec<(FitFile, String)>,
    /// Whether any of the inserted sessions holds a new personal record.
    pub new_prs: bool,
    /// Whether some exercise of the inserted sessions has a progression suggestion pending.
    pub imported_progression: bool,
    /// Whether any exercise has a progression suggestion pending, checked only when sessions
    /// were inserted (otherwise it cannot have changed).
    pub any_progression: Option<bool>,
}

/// Parses a batch of `.FIT` files in parallel and inserts each new session (plus its
/// exercises/series/heart rate/GPS/speeds) in the given transaction, then refreshes personal
/// records.
pub(crate) async fn import_file_list(
    tx: &mut Transaction<'_>,
    files: &[FitFile],
) -> Result<ImportedFiles, DatabaseError> {
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
                Either::Right((file.clone(), e.to_string()))
            }
        }
    });

    sessions.sort_by_key(|s| s.date);

    let dates: Vec<u32> = sessions.iter().map(|s| s.date).collect();
    let mut existing_sessions = Session::get_existing_dates(tx, &dates).await?;
    let known_devices = if sessions.is_empty() {
        HashSet::new()
    } else {
        Device::get_serials(tx).await?
    };
    let mut handled_workouts = HashSet::new();

    for session in &mut sessions {
        let formatted_time = match Local.timestamp_opt(session.date as i64, 0) {
            LocalResult::Single(date) => date.format("%H:%M:%S %d/%m/%Y").to_string(),
            _ => "".to_string(),
        };
        info!("Importing session {} - {}...", session.name, formatted_time);
        if existing_sessions.contains(&session.date) {
            warn!("Session with date {} already exists", session.date);
            continue;
        }

        // The serial recorded in the file may belong to a device that isn't registered
        session.device = session.device.take().filter(|s| known_devices.contains(s));
        Session::insert_full(tx, session, &mut handled_workouts).await?;

        handled_exercises.extend(
            session
                .steps
                .iter()
                .filter_map(|serie| serie.ex_cat.zip(serie.ex_id)),
        );
        existing_sessions.insert(session.date);
        success.push(session.date);
    }

    let mut new_prs = false;
    let mut imported_progression = false;
    let mut any_progression = None;
    if !success.is_empty() {
        new_prs = SessionStep::update_prs(tx, &handled_exercises, &success).await?;

        // Evaluated once: tells both whether the imported exercises progress and whether any does
        let suggested = progression_suggestions(tx).await?;
        imported_progression = handled_exercises.iter().any(|e| suggested.contains(e));
        any_progression = Some(!suggested.is_empty());
    }

    Ok(ImportedFiles {
        sessions: success,
        failed,
        new_prs,
        imported_progression,
        any_progression,
    })
}

/// Imports the `.fit` files fetched from `device` and, with `sync_mark`, moves its last activity
/// sync to it.
pub(super) async fn import_device_files(
    tx: &mut Transaction<'_>,
    files: &[FitFile],
    device: &mut Device,
    sync_mark: Option<u32>,
) -> Result<ImportedFiles, DatabaseError> {
    let res = if !files.is_empty() {
        info!("Fetched {} activity files", files.len());
        import_file_list(tx, files).await?
    } else {
        ImportedFiles::default()
    };

    if let Some(mark) = sync_mark {
        device.last_activity_sync = Some(mark);
        device.update_by_id_in(tx).await?;
    }

    Ok(res)
}

/// Outcome of `apply_session_changes`.
pub(crate) struct AppliedSessionChanges {
    /// Whether the edited session now holds a new personal record.
    pub new_prs: bool,
    /// Whether some exercise of the session got a new progression suggestion.
    pub newly_suggested: bool,
    /// Whether any exercise has a progression suggestion pending.
    pub any_progression: bool,
}

/// Stores the edited notes and sets of a session and recomputes the records of the exercises
/// involved.
pub(super) async fn apply_session_changes(
    tx: &Transaction<'_>,
    details: &SessionStepsUpdate,
) -> rusqlite_orm::errors::Result<AppliedSessionChanges> {
    SessionData::update_notes(tx, details.timestamp, details.notes.as_deref()).await?;

    let idxs: Vec<u8> = details.sets.iter().map(|s| s.idx).collect();
    let mut exercises = SessionStep::get_step_exercises(tx, details.timestamp, &idxs).await?;
    exercises.extend(details.sets.iter().filter_map(|s| s.ex_cat.zip(s.ex_id)));

    // Only tells whether a suggestion is new, so a failure just counts as none before
    let suggested_before = progression_suggestions_among(tx, &exercises)
        .await
        .unwrap_or_default();

    for serie in &details.sets {
        SessionStep::update_set(
            tx,
            details.timestamp,
            serie.idx,
            (serie.ex_cat, serie.ex_id),
            serie.reps,
            serie.weight,
        )
        .await?;
    }

    let new_prs = SessionStep::update_prs(tx, &exercises, &[details.timestamp]).await?;

    // Evaluated once: tells both which exercises now progress and whether any does
    let suggested_after = progression_suggestions(tx).await?;
    let newly_suggested = exercises
        .iter()
        .any(|ex| !suggested_before.contains(ex) && suggested_after.contains(ex));
    Ok(AppliedSessionChanges {
        new_prs,
        newly_suggested,
        any_progression: !suggested_after.is_empty(),
    })
}

/// Training load and records of every day of the last year, by month and day of month.
pub(crate) async fn heatmap_data(database: &DatabasePool) -> Result<Vec<Vec<(u32, u16)>>, String> {
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

    database
        .run_in_connection(async move |conn| {
            let sessions = Session::get_sessions(conn, Some(ts), None, None, None).await?;
            let pr_counts = SessionStep::get_pr_counts_since(conn, ts).await?;

            let mut res = vec![vec![(0_u32, 0_u16); 31]; 12];
            for session in sessions {
                let dt = Local
                    .timestamp_opt(session.date as i64, 0)
                    .single()
                    .unwrap();
                let cell = &mut res[dt.month0() as usize][dt.day0() as usize];
                cell.0 += session.training_load as u32;
                cell.1 += pr_counts.get(&session.date).copied().unwrap_or(0);
            }

            Ok(res)
        })
        .await
        .map_err(|e| e.to_string())
}

/// Merges the session started at `from_date` into the one at `to_date`: adds up their stats and
/// appends its laps, sets and additional data.
#[allow(dead_code)]
pub(super) async fn merge_sessions(
    database: &DatabasePool,
    to_date: u32,
    from_date: u32,
) -> Result<(), DatabaseError> {
    let (to, from) = database
        .run_in_connection(async move |conn| {
            let to = Session::get_full(conn, to_date).await?;
            let from = Session::get_full(conn, from_date).await?;
            Ok((to, from))
        })
        .await
        .expect("Error on database query");
    let mut to = to.unwrap_or_else(|| panic!("Could not find session with timestamp {}", to_date));
    let mut from =
        from.unwrap_or_else(|| panic!("Could not find session with timestamp {}", from_date));
    // Both sessions are already stored, so their workout must be left as it is
    to.workout_obj = None;

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

    database
        .run_in_transaction(async move |tx| {
            to.delete_by_id_in(tx).await?;
            from.delete_by_id_in(tx).await?;

            Session::insert_full(tx, &mut to, &mut HashSet::new()).await?;

            let handled_exercises = to
                .steps
                .iter()
                .filter_map(|s| s.ex_cat.zip(s.ex_id))
                .collect();
            SessionStep::update_prs(tx, &handled_exercises, &[to.date]).await?;

            Ok(())
        })
        .await
}

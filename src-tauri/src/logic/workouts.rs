use std::{
    collections::{HashMap, HashSet},
    process::Command,
};

use rusqlite_orm::{
    dao::Repository,
    types::{order_by::OrderBy, where_clause::Where},
};
use tauri::{AppHandle, Manager, Runtime, WebviewWindow, WebviewWindowBuilder};
use tauri_plugin_log::log::{error, info};

use crate::{
    ConnectLock, SettingsLock,
    dao::{
        exercise::{self, ExerciseRepository},
        session::{self, SessionRepository, entity},
        session_step::{self, SessionStepRepository},
        workout::{self, Workout, WorkoutRepository},
        workout_device::{self, WorkoutDeviceRepository},
        workout_step::{self, StepType, WorkoutStepRepository},
    },
    dto::{
        notifications::{NotificationDefinition, NotificationKind},
        workouts::{WorkoutDetails, WorkoutListItem, WorkoutSession},
    },
    logic::{
        ask_save_path,
        connect::sync_workouts_connect,
        devices::{
            delete_pending_from_connected, push_pending_to_usb,
            send_enabled_workout_to_connect_devices,
        },
        exercises::fetch_exercises_muscles,
        notifications::show_notification,
        report_error, run_blocking,
    },
    utils::{constants, translations::translate},
};

/// Returns sessions grouped/aggregated by workout name (count, average time, latest date), sorted by name.
pub async fn get_workout_list<R: Runtime>(
    app: &AppHandle<R>,
    settings: &SettingsLock,
) -> Result<Vec<WorkoutListItem>, String> {
    info!("Getting workouts list...");
    let res = run_blocking(app, move |database| {
        database.run_in_connection(|conn| {
            let workouts: Vec<_> = WorkoutRepository::select()
                .fetch_in(conn)?
                .into_iter()
                .collect();

            let sessions = SessionRepository::select()
                .columns(&[
                    entity::columns::NAME,
                    entity::columns::DATE,
                    entity::columns::TOTAL_ELAPSED_TIME,
                    entity::columns::TRAINING_LOAD,
                ])
                .where_(Where::NotNull(session::entity::columns::WORKOUT))
                .order_by(OrderBy::Desc(entity::columns::DATE))
                .fetch_in(conn)?;

            let mut workout_stats = HashMap::new();
            workouts.iter().for_each(|w| {
                workout_stats.insert(w.name.clone(), (0, 0, None, 0_u32));
            });

            let suggested_workouts: HashSet<_> = workouts
                .iter()
                .filter_map(|w| {
                    if w.enabled {
                        Some(w.name.clone())
                    } else {
                        None
                    }
                })
                .collect();

            sessions.iter().for_each(|s| {
                let entry = workout_stats
                    .entry(s.get_text(entity::columns::NAME.as_ref()))
                    .or_insert((0_u32, 0_u32, None, 0_u32));
                entry.0 += 1_u32;
                entry.1 += s.get_u32(entity::columns::TOTAL_ELAPSED_TIME.as_ref());
                if entry.2.is_none() {
                    entry.2 = Some(s.get_u32(entity::columns::DATE.as_ref()));
                }
                entry.3 += s.get_u32(entity::columns::TRAINING_LOAD.as_ref())
            });

            let with_steps: HashSet<_> = WorkoutStepRepository::select()
                .distinct(&[workout_step::entity::columns::WORKOUT])
                .fetch_in(conn)?
                .iter()
                .map(|r| r.get_text(workout_step::entity::columns::WORKOUT.as_ref()))
                .collect();

            let mut res: Vec<_> = workout_stats
                .into_iter()
                .map(|wd| WorkoutListItem {
                    has_steps: with_steps.contains(&wd.0),
                    enabled: suggested_workouts.contains(&wd.0),
                    name: wd.0,
                    sessions: wd.1.0,
                    avg_time: wd.1.1.checked_div(wd.1.0).unwrap_or(0),
                    avg_load: wd.1.3.checked_div(wd.1.0).unwrap_or(0) as u16,
                    latest_session: wd.1.2,
                })
                .collect();

            res.sort_by(|a, b| a.name.cmp(&b.name));

            Ok(res)
        })
    })
    .await;
    match res {
        Ok(l) => {
            info!("Retreived {} workouts", l.len());
            Ok(l)
        }
        Err(e) => Err(report_error(
            e,
            settings.read().unwrap().language,
            "error_workout_list",
            "Error getting workouts list",
        )),
    }
}

/// Returns every session for one workout name with per-session volume and session-over-session volume diff.
pub async fn get_workout_details<R: Runtime>(
    app: &AppHandle<R>,
    settings: &SettingsLock,
    name: &str,
) -> Result<WorkoutDetails, String> {
    let name = name.to_string();
    let res = run_blocking(app, move |database| {
        database.run_in_connection(|conn| {
            info!("Getting details for workout {}", name);

            let mut workout = WorkoutRepository::select_by_id_in(conn, &name)?.unwrap();
            workout.fetch_steps_relationship_in(conn)?;
            workout.steps.sort_by_key(|e| e.idx);

            let sessions = SessionRepository::select_by_name_in(
                conn,
                &name,
                Some(&[OrderBy::Desc(entity::columns::DATE)]),
            )?;

            let mut exercises = ExerciseRepository::select()
                .where_(Where::InMultipleSub(
                    vec![
                        exercise::entity::columns::CATEGORY,
                        exercise::entity::columns::ID,
                    ],
                    WorkoutStepRepository::select()
                        .distinct(&[
                            workout_step::entity::columns::EX_CAT,
                            workout_step::entity::columns::EX_ID,
                        ])
                        .where_(Where::Eq(
                            workout_step::entity::columns::WORKOUT,
                            workout.name.clone().into(),
                        ))
                        .to_subquery(),
                ))
                .fetch_in(conn)?;

            fetch_exercises_muscles(conn, &mut exercises)?;
            let mut primary_set = HashSet::new();
            let mut secondary_set = HashSet::new();
            for exercise in &exercises {
                for muscle in &exercise.muscles {
                    if muscle.is_primary {
                        primary_set.insert(muscle.muscle.clone());
                    } else {
                        secondary_set.insert(muscle.muscle.clone());
                    }
                }
            }

            let secondary_muscles = secondary_set
                .into_iter()
                .filter(|m| !primary_set.contains(m))
                .collect();
            let primary_muscles = primary_set.into_iter().collect();

            let latest = sessions.first().map(|latest| latest.date);
            let mut count = 0_u32;
            let mut time = 0_u32;
            let mut volume = 0_f32;

            let series = SessionStepRepository::select()
                .columns(&[
                    session_step::entity::columns::SESSION,
                    session_step::entity::columns::REPS,
                    session_step::entity::columns::WEIGHT,
                ])
                .where_(Where::And(vec![
                    // Rest steps have neither reps nor weight
                    Where::NotEq(session_step::entity::columns::KIND, StepType::Rest.into()),
                    Where::InSub(
                        session_step::entity::columns::SESSION,
                        SessionRepository::select()
                            .columns(&[entity::columns::DATE])
                            .where_(Where::Eq(entity::columns::NAME, name.clone().into()))
                            .to_subquery(),
                    ),
                ]))
                .fetch_in(conn)?;

            let mut volume_by_session = HashMap::new();
            for serie in &series {
                *volume_by_session
                    .entry(serie.get_u32(session_step::entity::columns::SESSION.as_ref()))
                    .or_insert(0.0) += serie
                    .get_u16_nullable(session_step::entity::columns::REPS.as_ref())
                    .unwrap_or_default() as f32
                    * serie
                        .get_f32_nullable(session_step::entity::columns::WEIGHT.as_ref())
                        .unwrap_or_default();
            }

            let mut session_list = Vec::new();
            for session in &sessions {
                count += 1;
                time += session.total_elapsed_time;

                let local_volume = volume_by_session.get(&session.date).copied().unwrap_or(0.0);
                volume += local_volume;
                let mut wk_sess = WorkoutSession::from(session);
                wk_sess.volume = local_volume;
                session_list.push(wk_sess);
            }

            let details = WorkoutDetails {
                name: name.to_string(),
                avg_time: if !sessions.is_empty() {
                    time / (sessions.len() as u32)
                } else {
                    0
                },
                latest_session: latest,
                avg_volume: volume / (sessions.len() as f32),
                session_count: count,
                sessions: session_list,
                enabled: workout.enabled,
                notes: workout.notes,
                steps: workout.steps,
                primary_muscles,
                secondary_muscles,
            };

            Ok(details)
        })
    })
    .await;

    match res {
        Ok(l) => {
            info!("Found details for workout {}", l.name);
            Ok(l)
        }
        Err(e) => Err(report_error(
            e,
            settings.read().unwrap().language,
            "error_workout_details",
            "Error getting workout details",
        )),
    }
}

pub async fn set_workout_status<R: Runtime>(
    app: &AppHandle<R>,
    workout: &str,
    status: bool,
) -> Result<(), String> {
    let name = workout;
    let workout = workout.to_string();
    let res = run_blocking(app, move |database| {
        database
            .run_in_transaction(|tx| {
                let wk_cl = workout.clone();

                let res = WorkoutRepository::update()
                    .where_(Where::Eq(
                        workout::entity::columns::NAME,
                        workout.clone().into(),
                    ))
                    .set(workout::entity::columns::ENABLED, status.into())
                    .execute_in(tx)
                    .map(|_| ());

                if res.is_ok() {
                    // The enabled flag only exists on devices, so Connect is left as it was
                    let _ = WorkoutDeviceRepository::update()
                        .set(workout_device::entity::columns::PENDING, true.into())
                        .where_(Where::And(vec![
                            Where::Eq(workout_device::entity::columns::WORKOUT, wk_cl.into()),
                            Where::NotEq(
                                workout_device::entity::columns::DEVICE,
                                constants::CONNECT_DEVICE_SERIAL.into(),
                            ),
                        ]))
                        .execute_in(tx);
                }

                Ok(res?)
            })
            .map_err(|e| e.to_string())
    })
    .await;

    if res.is_ok() {
        if status {
            push_pending_to_usb(app).await;
            send_enabled_workout_to_connect_devices(app, name).await;
        } else {
            delete_pending_from_connected(app).await;
        }
    }

    res
}

pub async fn save_workout<R: Runtime>(
    app: &AppHandle<R>,
    settings: &SettingsLock,
    connect: &ConnectLock,
    edit: bool,
    workout: Workout,
) -> Result<(), String> {
    let name = workout.name.clone();
    let name_cl = workout.name.clone();
    let res = run_blocking(app, move |database| {
        database.run_in_transaction(|tx| {
            if edit && WorkoutRepository::select_by_id_in(tx, &workout.name)?.is_some() {
                WorkoutRepository::update()
                    .set(
                        workout::entity::columns::NOTES,
                        (if let Some(note) = workout.notes.clone()
                            && !note.is_empty()
                        {
                            Some(note)
                        } else {
                            None
                        })
                        .into(),
                    )
                    .where_(Where::Eq(
                        workout::entity::columns::NAME,
                        workout.name.clone().into(),
                    ))
                    .execute_in(tx)?;

                WorkoutStepRepository::delete()
                    .where_(Where::Eq(
                        workout_step::entity::columns::WORKOUT,
                        workout.name.clone().into(),
                    ))
                    .execute_in(tx)?;

                // Re-send the edited workout to every device that already has it
                WorkoutDeviceRepository::update()
                    .set(workout_device::entity::columns::PENDING, true.into())
                    .where_(Where::Eq(
                        workout_device::entity::columns::WORKOUT,
                        workout.name.clone().into(),
                    ))
                    .execute_in(tx)?;
            } else {
                let mut item = workout.clone();
                WorkoutRepository::insert().item(&mut item).execute_in(tx)?;
            }

            let mut insert = WorkoutStepRepository::insert();
            let mut steps = workout.steps.clone();
            for step in &mut steps {
                step.workout = workout.name.clone();
                // Empty notes are stored as NULL so the exercise notes fallback applies on upload
                if step.notes.as_deref().is_some_and(str::is_empty) {
                    step.notes = None;
                }
                insert = insert.item(step);
            }
            insert.execute_in(tx)?;

            Ok(())
        })
    })
    .await;

    let lang = settings.read().unwrap().language;
    match res {
        Ok(_) => {
            info!("Workout '{}' saved succesfully", name);
            show_notification(NotificationDefinition {
                title: name,
                body: translate("ok_workout_save", lang),
                kind: NotificationKind::Temporal,
            });
            sync_workouts_connect(app, connect, settings, &[name_cl]).await;
            if edit {
                push_pending_to_usb(app).await;
            }
            Ok(())
        }
        Err(e) => Err(report_error(
            e,
            lang,
            "error_workout_save",
            "Error saving workout",
        )),
    }
}

pub async fn load_workout_page<R: Runtime>(
    app: &AppHandle<R>,
    workout: &str,
) -> Result<(), String> {
    let mut config = app
        .config()
        .app
        .windows
        .iter()
        .find(|w| w.label == "workout")
        .expect("no hay ventana 'workout' en tauri.conf.json")
        .clone();

    config.url = tauri::WebviewUrl::App(format!("{}?workout={workout}", config.url).into());

    WebviewWindowBuilder::from_config(app, &config)
        .map_err(|e| e.to_string())?
        .build()
        .map_err(|e| e.to_string())?;

    Ok(())
}

/// Called by the report window once everything is rendered and painted: prints it to PDF.
pub async fn workout_print_ready(window: WebviewWindow, workout: &str) -> Result<(), String> {
    info!("Report ready, printing");
    let res = print_to_pdf(window.clone(), workout).await;
    // On success the window is closed once the print operation ends
    if res.is_err() {
        let _ = window.close();
    }
    res
}

async fn print_to_pdf(w: WebviewWindow, workout: &str) -> Result<(), String> {
    use webkit2gtk::{PrintOperation, PrintOperationExt};

    let lang = w.state::<SettingsLock>().read().unwrap().language;
    let default_file = w
        .path()
        .download_dir()
        .map_err(|e| e.to_string())?
        .join(format!("{}.pdf", workout));

    let Some(file) = ask_save_path(
        w.app_handle(),
        &default_file,
        "PDF",
        "pdf",
        &translate("select_file_save_pdf", lang),
    )
    .await?
    else {
        // Cancelled by the user
        let _ = w.close();
        return Ok(());
    };

    let uri = format!("file://{}", file.display());
    w.clone()
        .with_webview(move |pw| {
            let settings = gtk::PrintSettings::new();
            // GTK registers its file backend under a localized name
            settings.set_printer(&gtk::glib::dgettext(Some("gtk30"), "Print to File"));
            settings.set(gtk::PRINT_SETTINGS_OUTPUT_FILE_FORMAT, Some("pdf"));
            settings.set(gtk::PRINT_SETTINGS_OUTPUT_URI, Some(&uri));

            let page_setup = gtk::PageSetup::new();
            page_setup.set_paper_size(&gtk::PaperSize::new(Some(gtk::PAPER_NAME_A4)));
            page_setup.set_top_margin(15.0, gtk::Unit::Mm);
            page_setup.set_bottom_margin(15.0, gtk::Unit::Mm);
            page_setup.set_left_margin(15.0, gtk::Unit::Mm);
            page_setup.set_right_margin(15.0, gtk::Unit::Mm);

            let op = PrintOperation::new(&pw.inner());
            op.set_print_settings(&settings);
            op.set_page_setup(&page_setup);
            op.connect_failed(move |_, e| {
                error!("PDF print failed: {e}");
            });
            // Emitted after `failed` too, so the hidden window is always released here
            op.connect_finished(move |_| {
                info!("PDF printed to {uri}");
                let _ = Command::new("xdg-open").arg(&uri).status();
                let _ = w.close();
            });
            op.print();
        })
        .map_err(|e| e.to_string())
}

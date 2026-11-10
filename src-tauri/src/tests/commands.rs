use rusqlite_orm::{dao::Repository, database::DatabasePool};
use rustyfit::profile::typedef::Intensity;
use tauri::{
    App, Manager,
    async_runtime::block_on,
    test::{MockRuntime, mock_app},
};

use crate::{
    ConnectLock, MtpLock, SettingsLock,
    connect::ConnectClient,
    dao::{
        device::{Device, DeviceRepository},
        session::Session,
        settings::WeightUnit,
        settings::settings_keys::CLOSE_TO_TRAY,
        workout::Workout,
        workout_device::WorkoutDeviceRepository,
        workout_step::{StepType, WorkoutStep},
    },
    dto::prompts::PromptKind,
    dto::{
        app::Settings,
        body_metrics::BodyMetricListItem,
        exercises::ProgressionSuggestion,
        sessions::{SessionStepDto, SessionStepsUpdate},
    },
    fit::parser::FitParser,
    logic::{
        app, body_metrics, connect, devices, exercises,
        prompts::{
            self,
            helpers::{AppPrompter, Prompts},
        },
        report, sessions, workouts,
    },
    mtp::MtpClient,
    tests::common,
    utils::{constants, translations::Languages},
};

const BENCH: (u16, u16) = (0, 1);

/// App on the mock runtime with the state of the real one, over an in-memory database and without
/// reaching the keyring nor the USB devices.
fn test_app() -> App<MockRuntime> {
    let app = mock_app();
    let db = common::test_db();
    app.manage(SettingsLock::new(Settings::from(&db)));
    app.manage(db);
    app.manage(Prompts::default());
    let prompter = AppPrompter(app.handle().clone());
    app.manage(ConnectLock::new(ConnectClient::new(|_| {}, prompter)));
    app.manage(MtpLock::new(MtpClient {}));
    app
}

fn step(idx: u16, kind: Intensity, weight: f32, reps: u16) -> WorkoutStep {
    WorkoutStep::active("", idx, kind, Some(BENCH), weight, Some(reps), None, None)
}

fn workout(name: &str, steps: Vec<WorkoutStep>) -> Workout {
    Workout {
        name: name.to_string(),
        enabled: true,
        notes: None,
        connect_id: None,
        steps,
    }
}

fn save_workout(app: &App<MockRuntime>, edit: bool, workout: Workout) {
    block_on(workouts::save_workout(
        app.handle(),
        &app.state::<SettingsLock>(),
        &app.state::<ConnectLock>(),
        edit,
        workout,
    ))
    .unwrap();
}

fn workout_steps(app: &App<MockRuntime>, name: &str) -> Vec<WorkoutStep> {
    block_on(workouts::get_workout_details(
        app.handle(),
        &app.state::<SettingsLock>(),
        name,
    ))
    .unwrap()
    .steps
}

fn insert_session(app: &App<MockRuntime>, date: u32, reps: u16, weight: f32) {
    app.state::<DatabasePool>()
        .run_in_transaction(|tx| {
            let mut session = common::session(date);
            session.steps = vec![common::set(date, 0, BENCH, reps, weight)];
            common::insert_session(tx, session);
            Ok(())
        })
        .unwrap();
}

// App

#[test]
fn settings_are_read_and_updated_in_memory_and_database() {
    let app = test_app();
    block_on(app::update_settings_value(
        app.handle(),
        CLOSE_TO_TRAY,
        "true",
    ))
    .unwrap();

    let settings = block_on(app::get_settings(&app.state::<SettingsLock>())).unwrap();
    assert!(settings.close_to_tray);
    assert!(Settings::from(&*app.state::<DatabasePool>()).close_to_tray);
}

#[test]
fn translations_contain_the_step_kinds() {
    let app = test_app();
    let translations = block_on(app::get_translations(&app.state::<SettingsLock>())).unwrap();

    for key in [
        "step_type_warmup",
        "step_type_cooldown",
        "step_type_recovery",
        "step_type_other",
    ] {
        assert!(translations.contains_key(key), "{key}");
    }
}

// Body metrics

#[test]
fn body_measures_are_added_listed_and_deleted() {
    let app = test_app();
    let measures = BodyMetricListItem {
        date: 1_700_000_000,
        weight: 80.0,
        fat_ratio: 15.0,
        lean_mass: 60.0,
        water_ratio: 55.0,
    };
    let settings = app.state::<SettingsLock>();

    block_on(body_metrics::add_body_measures(
        app.handle(),
        &settings,
        measures,
    ))
    .unwrap();
    let listed = block_on(body_metrics::get_body_measures(app.handle(), &settings)).unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].weight, 80.0);

    // Stored at local midnight, the listed date is the one to delete by
    block_on(body_metrics::delete_body_metric(
        app.handle(),
        &settings,
        listed[0].date,
    ))
    .unwrap();
    assert!(
        block_on(body_metrics::get_body_measures(app.handle(), &settings))
            .unwrap()
            .is_empty()
    );
}

// Connect

#[test]
fn connect_starts_logged_out_and_logout_never_fails() {
    let app = test_app();
    let client = app.state::<ConnectLock>();

    assert_eq!(block_on(connect::connect_logged_user(&client)), Ok(None));
    assert_eq!(block_on(connect::connect_logout(&client)), Ok(()));
}

// Devices

#[test]
fn workout_for_an_unplugged_device_is_left_pending() {
    let app = test_app();
    save_workout(
        &app,
        false,
        workout("Push", vec![step(0, Intensity::ACTIVE, 60.0, 10)]),
    );
    app.state::<DatabasePool>()
        .run_in_transaction(|tx| {
            DeviceRepository::insert()
                .item(&mut Device {
                    serial: "watch".to_string(),
                    model: "model".to_string(),
                    last_activity_sync: None,
                    last_body_metric_sync: None,
                    usb: true,
                    connect: false,
                    strength_workout: true,
                })
                .execute_in(tx)?;
            Ok(())
        })
        .unwrap();

    block_on(devices::send_to_device(
        app.handle(),
        &app.state::<SettingsLock>(),
        "Push",
        "watch",
    ))
    .unwrap();

    let entries = WorkoutDeviceRepository::select()
        .fetch(&app.state::<DatabasePool>())
        .unwrap();
    assert_eq!(entries.len(), 2);
    assert!(entries.iter().all(|e| e.pending));
    // Saved while logged out, so it is also left pending of upload to Connect
    assert!(entries.iter().any(|e| e.device == "watch"));
    assert!(
        entries
            .iter()
            .any(|e| e.device == constants::CONNECT_DEVICE_SERIAL)
    );

    let registered = block_on(devices::get_registered_devices(app.handle())).unwrap();
    assert_eq!(registered.len(), 1);
}

// Exercises

#[test]
fn applying_a_progression_leaves_warmups_untouched() {
    let app = test_app();
    save_workout(
        &app,
        false,
        workout(
            "Push",
            vec![
                step(0, Intensity::WARMUP, 40.0, 15),
                step(1, Intensity::ACTIVE, 100.0, 8),
            ],
        ),
    );

    block_on(exercises::apply_progression_suggestion(
        app.handle(),
        &app.state::<SettingsLock>(),
        &app.state::<ConnectLock>(),
        BENCH.0,
        BENCH.1,
        ProgressionSuggestion {
            weight: 102.0,
            reps: 8,
        },
    ))
    .unwrap();

    let steps = workout_steps(&app, "Push");
    assert_eq!(steps[0].kind, StepType::Warmup);
    assert_eq!(steps[0].weight, Some(40.0));
    assert_eq!(steps[0].reps.flatten(), Some(15));
    assert_eq!(steps[1].weight, Some(102.0));
}

#[test]
fn progression_status_ignores_warmups() {
    let app = test_app();
    save_workout(
        &app,
        false,
        workout(
            "Push",
            vec![
                step(0, Intensity::WARMUP, 120.0, 15),
                step(1, Intensity::ACTIVE, 100.0, 8),
            ],
        ),
    );

    let status = block_on(exercises::get_progression_status(
        app.handle(),
        BENCH.0,
        BENCH.1,
    ))
    .unwrap();
    assert_eq!(
        status,
        Some(ProgressionSuggestion {
            weight: 100.0,
            reps: 8,
        })
    );
}

#[test]
fn exercise_notes_are_trimmed_and_empty_ones_removed() {
    let app = test_app();
    let settings = app.state::<SettingsLock>();
    let client = app.state::<ConnectLock>();
    let save = |notes| {
        block_on(exercises::save_exercise_notes(
            app.handle(),
            &settings,
            &client,
            BENCH.0,
            BENCH.1,
            notes,
        ))
        .unwrap();
        block_on(exercises::get_exercise_details(
            app.handle(),
            &settings,
            BENCH.0,
            BENCH.1,
        ))
        .unwrap()
        .notes
    };

    assert_eq!(save("  Slow  ").as_deref(), Some("Slow"));
    assert_eq!(save("   "), None);
}

#[test]
fn catalog_lists_the_seeded_exercises() {
    let app = test_app();
    let catalog = block_on(exercises::get_exercises_catalog(app.handle())).unwrap();

    assert!(catalog[&BENCH.0].contains(&BENCH.1));
}

// Report

#[test]
fn report_of_an_empty_range_is_empty() {
    let app = test_app();
    insert_session(&app, 1_700_000_000, 10, 60.0);

    let data = block_on(report::get_report_data(
        app.handle(),
        &app.state::<SettingsLock>(),
        1,
        2,
    ))
    .unwrap();

    assert!(data.sessions.is_empty());
    assert!(data.exercises.is_empty());
    assert!(data.body_metrics.is_empty());
}

// Sessions

#[test]
fn session_changes_are_saved_and_records_recomputed() {
    let app = test_app();
    let date = 1_700_000_000;
    insert_session(&app, date, 10, 60.0);
    let settings = app.state::<SettingsLock>();

    let listed = block_on(sessions::get_sessions(app.handle(), &settings, None)).unwrap();
    assert_eq!(listed.len(), 1);

    let details = block_on(sessions::get_session_details(app.handle(), &settings, date)).unwrap();
    let mut sets: Vec<SessionStepDto> = details.steps;
    sets[0].weight = Some(70.0);

    block_on(sessions::save_session_changes(
        app.handle(),
        &settings,
        SessionStepsUpdate {
            timestamp: date,
            sets,
            notes: Some("Felt strong".to_string()),
        },
    ))
    .unwrap();

    let details = block_on(sessions::get_session_details(app.handle(), &settings, date)).unwrap();
    assert_eq!(details.steps[0].weight, Some(70.0));
    assert_eq!(details.notes, "Felt strong");

    let records = block_on(exercises::get_exercises(app.handle(), &settings)).unwrap();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].weight, 70.0);
}

#[test]
fn heatmap_is_available_without_sessions() {
    let app = test_app();

    assert!(block_on(sessions::get_heatmap_data(app.handle())).is_ok());
}

// Workouts

#[test]
fn workouts_are_saved_edited_and_toggled() {
    let app = test_app();
    let settings = app.state::<SettingsLock>();
    save_workout(
        &app,
        false,
        workout("Push", vec![step(0, Intensity::ACTIVE, 60.0, 10)]),
    );
    save_workout(
        &app,
        true,
        workout(
            "Push",
            vec![
                step(0, Intensity::WARMUP, 20.0, 15),
                step(1, Intensity::ACTIVE, 60.0, 10),
            ],
        ),
    );
    assert_eq!(workout_steps(&app, "Push").len(), 2);

    block_on(workouts::set_workout_status(app.handle(), "Push", false)).unwrap();

    let listed = block_on(workouts::get_workout_list(app.handle(), &settings)).unwrap();
    assert_eq!(listed.len(), 1);
    assert!(!listed[0].enabled);
    assert!(listed[0].has_steps);
}

#[test]
fn stored_session_is_written_as_fit() {
    let app = test_app();
    let date = 1_700_000_000;
    insert_session(&app, date, 10, 60.0);
    let path = std::env::temp_dir().join(format!(
        "gtrs-test-session-download-{}.fit",
        std::process::id()
    ));

    block_on(sessions::helpers::write_session_fit(
        app.handle(),
        date,
        path.clone(),
        Languages::English,
        WeightUnit::Kilograms,
    ))
    .unwrap();
    let written = FitParser::from_file(&path).and_then(Session::try_from);
    let _ = std::fs::remove_file(&path);
    let written = written.unwrap();

    assert_eq!(written.date, date);
    assert_eq!(written.steps.len(), 1);
    assert_eq!(written.steps[0].reps, Some(10));
    assert_eq!(written.steps[0].weight, Some(60.0));
    assert_eq!(written.steps[0].ex_cat, Some(BENCH.0));
    assert_eq!(written.steps[0].ex_id, Some(BENCH.1));
}

#[test]
fn missing_session_fails_to_write_as_fit() {
    let app = test_app();
    let path = std::env::temp_dir().join(format!(
        "gtrs-test-session-download-missing-{}.fit",
        std::process::id()
    ));

    let res = block_on(sessions::helpers::write_session_fit(
        app.handle(),
        1,
        path.clone(),
        Languages::English,
        WeightUnit::Kilograms,
    ));

    assert!(res.is_err());
    assert!(!path.exists());
}

#[test]
fn session_file_name_is_date_then_name() {
    let date = chrono::TimeZone::with_ymd_and_hms(&chrono::Local, 2026, 11, 4, 10, 37, 58)
        .unwrap()
        .timestamp() as u32;

    assert_eq!(
        sessions::helpers::session_file_name(date, "Progresion 1 - Pierna"),
        "2026-11-04 Progresion 1 - Pierna.fit"
    );
    assert_eq!(
        sessions::helpers::session_file_name(date, "Push/Pull"),
        "2026-11-04 Push-Pull.fit"
    );
    assert_eq!(
        sessions::helpers::session_file_name(date, "  "),
        "2026-11-04.fit"
    );
}

#[test]
fn prompt_resolves_with_the_answer() {
    let app = test_app();
    let handle = app.handle().clone();

    let pending = tauri::async_runtime::spawn(async move {
        prompts::helpers::request_prompt(&handle, PromptKind::ConnectMfa, None, None).await
    });
    // The first prompt gets id 0, answered once the request has registered it
    while prompts::answer_prompt(&app.state::<Prompts>(), 0, Some(vec!["123456".into()])).is_err() {
        std::thread::yield_now();
    }

    assert_eq!(block_on(pending).unwrap(), Some(vec!["123456".to_string()]));
}

#[test]
fn prompt_resolves_to_none_when_cancelled() {
    let app = test_app();
    let handle = app.handle().clone();

    let pending = tauri::async_runtime::spawn(async move {
        prompts::helpers::request_prompt(&handle, PromptKind::SudoPassword, None, None).await
    });
    while prompts::answer_prompt(&app.state::<Prompts>(), 0, None).is_err() {
        std::thread::yield_now();
    }

    assert_eq!(block_on(pending).unwrap(), None);
}

#[test]
fn answering_an_unknown_prompt_fails() {
    let app = test_app();
    assert!(prompts::answer_prompt(&app.state::<Prompts>(), 42, None).is_err());
}

use rusqlite_orm::{dao::Repository, database::DatabasePool};
use rustyfit::profile::typedef::Intensity;
use tauri::{
    App, Manager,
    test::{MockRuntime, mock_app},
};

use crate::{
    ConnectLock, MtpLock, SettingsLock,
    connect::ConnectClient,
    dao::{
        device::{Device, DeviceRepository},
        session::Session,
        settings::settings_keys::DISTANCE_UNIT,
        settings::{DistanceUnit, WeightUnit},
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
async fn test_app() -> App<MockRuntime> {
    let app = mock_app();
    let db = common::test_db().await;
    app.manage(SettingsLock::new(Settings::load(&db).await));
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

async fn save_workout(app: &App<MockRuntime>, edit: bool, workout: Workout) {
    workouts::save_workout(
        app.handle(),
        &app.state::<SettingsLock>(),
        &app.state::<ConnectLock>(),
        edit,
        workout,
    )
    .await
    .unwrap();
}

async fn workout_steps(app: &App<MockRuntime>, name: &str) -> Vec<WorkoutStep> {
    workouts::get_workout_details(app.handle(), &app.state::<SettingsLock>(), name)
        .await
        .unwrap()
        .steps
}

async fn insert_session(app: &App<MockRuntime>, date: u32, reps: u16, weight: f32) {
    app.state::<DatabasePool>()
        .run_in_transaction(move |tx| {
            let mut session = common::session(date);
            session.steps = vec![common::set(date, 0, BENCH, reps, weight)];
            common::insert_session(tx, session);
            Ok(())
        })
        .await
        .unwrap();
}

// App

#[tokio::test]
async fn settings_are_read_and_updated_in_memory_and_database() {
    let app = test_app().await;
    app::update_settings_value(app.handle(), DISTANCE_UNIT, "Miles")
        .await
        .unwrap();

    let settings = app::get_settings(&app.state::<SettingsLock>())
        .await
        .unwrap();
    assert!(matches!(settings.distance_unit, DistanceUnit::Miles));
    assert!(matches!(
        Settings::load(&app.state::<DatabasePool>())
            .await
            .distance_unit,
        DistanceUnit::Miles
    ));
}

#[tokio::test]
async fn translations_contain_the_step_kinds() {
    let app = test_app().await;
    let translations = app::get_translations(&app.state::<SettingsLock>())
        .await
        .unwrap();

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

#[tokio::test]
async fn body_measures_are_added_listed_and_deleted() {
    let app = test_app().await;
    let measures = BodyMetricListItem {
        date: 1_700_000_000,
        weight: 80.0,
        fat_ratio: 15.0,
        lean_mass: 60.0,
        water_ratio: 55.0,
    };
    let settings = app.state::<SettingsLock>();

    body_metrics::add_body_measures(app.handle(), &settings, measures)
        .await
        .unwrap();
    let listed = body_metrics::get_body_measures(app.handle(), &settings)
        .await
        .unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].weight, 80.0);

    // Stored at local midnight, the listed date is the one to delete by
    body_metrics::delete_body_metric(app.handle(), &settings, listed[0].date)
        .await
        .unwrap();
    assert!(
        body_metrics::get_body_measures(app.handle(), &settings)
            .await
            .unwrap()
            .is_empty()
    );
}

// Connect

#[tokio::test]
async fn connect_starts_logged_out_and_logout_never_fails() {
    let app = test_app().await;
    let client = app.state::<ConnectLock>();

    assert_eq!(connect::connect_logged_user(&client).await, Ok(None));
    assert_eq!(connect::connect_logout(&client).await, Ok(()));
}

// Devices

#[tokio::test]
async fn workout_for_an_unplugged_device_is_left_pending() {
    let app = test_app().await;
    save_workout(
        &app,
        false,
        workout("Push", vec![step(0, Intensity::ACTIVE, 60.0, 10)]),
    )
    .await;
    app.state::<DatabasePool>()
        .run_in_transaction(move |tx| {
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
        .await
        .unwrap();

    devices::send_to_device(app.handle(), &app.state::<SettingsLock>(), "Push", "watch")
        .await
        .unwrap();

    let entries = WorkoutDeviceRepository::select()
        .fetch(&app.state::<DatabasePool>())
        .await
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

    let registered = devices::get_registered_devices(app.handle()).await.unwrap();
    assert_eq!(registered.len(), 1);
}

// Exercises

#[tokio::test]
async fn applying_a_progression_leaves_warmups_untouched() {
    let app = test_app().await;
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
    )
    .await;

    exercises::apply_progression_suggestion(
        app.handle(),
        &app.state::<SettingsLock>(),
        &app.state::<ConnectLock>(),
        BENCH.0,
        BENCH.1,
        ProgressionSuggestion {
            weight: 102.0,
            reps: 8,
        },
    )
    .await
    .unwrap();

    let steps = workout_steps(&app, "Push").await;
    assert_eq!(steps[0].kind, StepType::Warmup);
    assert_eq!(steps[0].weight, Some(40.0));
    assert_eq!(steps[0].reps.flatten(), Some(15));
    assert_eq!(steps[1].weight, Some(102.0));
}

#[tokio::test]
async fn progression_status_ignores_warmups() {
    let app = test_app().await;
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
    )
    .await;

    let status = exercises::get_progression_status(app.handle(), BENCH.0, BENCH.1)
        .await
        .unwrap();
    assert_eq!(
        status,
        Some(ProgressionSuggestion {
            weight: 100.0,
            reps: 8,
        })
    );
}

#[tokio::test]
async fn exercise_notes_are_trimmed_and_empty_ones_removed() {
    let app = test_app().await;
    let settings = app.state::<SettingsLock>();
    let client = app.state::<ConnectLock>();
    let save = async |notes: &str| {
        exercises::save_exercise_notes(app.handle(), &settings, &client, BENCH.0, BENCH.1, notes)
            .await
            .unwrap();
        exercises::get_exercise_details(app.handle(), &settings, BENCH.0, BENCH.1)
            .await
            .unwrap()
            .notes
    };

    assert_eq!(save("  Slow  ").await.as_deref(), Some("Slow"));
    assert_eq!(save("   ").await, None);
}

#[tokio::test]
async fn catalog_lists_the_seeded_exercises() {
    let app = test_app().await;
    let catalog = exercises::get_exercises_catalog(app.handle())
        .await
        .unwrap();

    assert!(catalog[&BENCH.0].contains(&BENCH.1));
}

// Report

#[tokio::test]
async fn report_of_an_empty_range_is_empty() {
    let app = test_app().await;
    insert_session(&app, 1_700_000_000, 10, 60.0).await;

    let data = report::get_report_data(app.handle(), &app.state::<SettingsLock>(), 1, 2)
        .await
        .unwrap();

    assert!(data.sessions.is_empty());
    assert!(data.exercises.is_empty());
    assert!(data.body_metrics.is_empty());
}

// Sessions

#[tokio::test]
async fn session_changes_are_saved_and_records_recomputed() {
    let app = test_app().await;
    let date = 1_700_000_000;
    insert_session(&app, date, 10, 60.0).await;
    let settings = app.state::<SettingsLock>();

    let listed = sessions::get_sessions(app.handle(), &settings, None)
        .await
        .unwrap();
    assert_eq!(listed.len(), 1);

    let details = sessions::get_session_details(app.handle(), &settings, date)
        .await
        .unwrap();
    let mut sets: Vec<SessionStepDto> = details.steps;
    sets[0].weight = Some(70.0);

    sessions::save_session_changes(
        app.handle(),
        &settings,
        SessionStepsUpdate {
            timestamp: date,
            sets,
            notes: Some("Felt strong".to_string()),
        },
    )
    .await
    .unwrap();

    let details = sessions::get_session_details(app.handle(), &settings, date)
        .await
        .unwrap();
    assert_eq!(details.steps[0].weight, Some(70.0));
    assert_eq!(details.notes, "Felt strong");

    let records = exercises::get_exercises(app.handle(), &settings)
        .await
        .unwrap();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].weight, 70.0);
}

#[tokio::test]
async fn heatmap_is_available_without_sessions() {
    let app = test_app().await;

    assert!(sessions::get_heatmap_data(app.handle()).await.is_ok());
}

// Workouts

#[tokio::test]
async fn workouts_are_saved_edited_and_toggled() {
    let app = test_app().await;
    let settings = app.state::<SettingsLock>();
    save_workout(
        &app,
        false,
        workout("Push", vec![step(0, Intensity::ACTIVE, 60.0, 10)]),
    )
    .await;
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
    )
    .await;
    assert_eq!(workout_steps(&app, "Push").await.len(), 2);

    workouts::set_workout_status(app.handle(), "Push", false)
        .await
        .unwrap();

    let listed = workouts::get_workout_list(app.handle(), &settings)
        .await
        .unwrap();
    assert_eq!(listed.len(), 1);
    assert!(!listed[0].enabled);
    assert!(listed[0].has_steps);
}

#[tokio::test]
async fn stored_session_is_written_as_fit() {
    let app = test_app().await;
    let date = 1_700_000_000;
    insert_session(&app, date, 10, 60.0).await;
    let path = std::env::temp_dir().join(format!(
        "gtrs-test-session-download-{}.fit",
        std::process::id()
    ));

    sessions::helpers::write_session_fit(
        app.handle(),
        date,
        path.clone(),
        Languages::English,
        WeightUnit::Kilograms,
    )
    .await
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

#[tokio::test]
async fn missing_session_fails_to_write_as_fit() {
    let app = test_app().await;
    let path = std::env::temp_dir().join(format!(
        "gtrs-test-session-download-missing-{}.fit",
        std::process::id()
    ));

    let res = sessions::helpers::write_session_fit(
        app.handle(),
        1,
        path.clone(),
        Languages::English,
        WeightUnit::Kilograms,
    )
    .await;

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

#[tokio::test]
async fn prompt_resolves_with_the_answer() {
    let app = test_app().await;
    let handle = app.handle().clone();

    let pending = tauri::async_runtime::spawn(async move {
        prompts::helpers::request_prompt(&handle, PromptKind::ConnectMfa, None, None).await
    });
    // The first prompt gets id 0, answered once the request has registered it
    while prompts::answer_prompt(&app.state::<Prompts>(), 0, Some(vec!["123456".into()])).is_err() {
        std::thread::yield_now();
    }

    assert_eq!(pending.await.unwrap(), Some(vec!["123456".to_string()]));
}

#[tokio::test]
async fn prompt_resolves_to_none_when_cancelled() {
    let app = test_app().await;
    let handle = app.handle().clone();

    let pending = tauri::async_runtime::spawn(async move {
        prompts::helpers::request_prompt(&handle, PromptKind::SudoPassword, None, None).await
    });
    while prompts::answer_prompt(&app.state::<Prompts>(), 0, None).is_err() {
        std::thread::yield_now();
    }

    assert_eq!(pending.await.unwrap(), None);
}

#[tokio::test]
async fn answering_an_unknown_prompt_fails() {
    let app = test_app().await;
    assert!(prompts::answer_prompt(&app.state::<Prompts>(), 42, None).is_err());
}

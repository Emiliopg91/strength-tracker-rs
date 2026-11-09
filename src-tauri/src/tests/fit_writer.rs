use rustyfit::profile::{
    mesgdef,
    typedef::{Intensity, MesgNum},
};

use crate::{
    dao::{
        session::Session,
        settings::WeightUnit,
        workout::Workout,
        workout_step::{StepType, WorkoutStep},
    },
    fit::{
        parser::FitParser,
        writer::{FitWriter, ToFitMessages},
    },
    utils::translations::Languages,
};

fn written_intensity(kind: Intensity) -> Intensity {
    let workout = Workout {
        name: "Push".to_string(),
        enabled: true,
        notes: None,
        connect_id: None,
        steps: vec![WorkoutStep::active(
            "Push",
            0,
            kind,
            Some((0, 1)),
            60.0,
            Some(10),
            None,
            None,
        )],
    };

    let messages = workout
        .to_fit_messages(Languages::English, WeightUnit::Kilograms)
        .unwrap();
    let step = messages
        .into_iter()
        .find(|msg| msg.num == MesgNum::WORKOUT_STEP)
        .unwrap();

    mesgdef::WorkoutStep::from(&step).intensity
}

#[test]
fn step_kinds_survive_a_fit_round_trip() {
    let kinds = [
        (Intensity::ACTIVE, StepType::Exercise),
        (Intensity::WARMUP, StepType::Warmup),
        (Intensity::COOLDOWN, StepType::Cooldown),
        (Intensity::RECOVERY, StepType::Recovery),
        (Intensity::OTHER, StepType::Other),
    ];
    for (intensity, kind) in kinds {
        let written = written_intensity(intensity);

        assert_eq!(written, intensity);
        assert_eq!(StepType::from(written), kind);
    }
}

#[test]
fn step_without_exercise_writes_no_exercise_category() {
    let workout = Workout {
        name: "Push".to_string(),
        enabled: true,
        notes: None,
        connect_id: None,
        steps: vec![WorkoutStep::active(
            "Push",
            0,
            Intensity::WARMUP,
            None,
            0.0,
            None,
            None,
            None,
        )],
    };

    let messages = workout
        .to_fit_messages(Languages::English, WeightUnit::Kilograms)
        .unwrap();
    let step = messages
        .iter()
        .find(|msg| msg.num == MesgNum::WORKOUT_STEP)
        .unwrap();

    assert_eq!(
        mesgdef::WorkoutStep::from(step).exercise_category.0,
        u16::MAX
    );
}

fn parse_file(path: &std::path::Path) -> Session {
    FitParser::from_file(path)
        .and_then(Session::try_from)
        .unwrap_or_else(|e| panic!("{}: {e:?}", path.display()))
}

fn parse_fixture(name: &str) -> Session {
    parse_file(std::path::Path::new(&format!(
        "{}/src/tests/fixtures/{name}.fit",
        env!("CARGO_MANIFEST_DIR")
    )))
}

fn round_trip(session: &Session, name: &str) -> Session {
    let path = std::env::temp_dir().join(format!(
        "gtrs-test-session-writer-{name}-{}.fit",
        std::process::id()
    ));
    FitWriter::from(session)
        .write(&path, Languages::English, WeightUnit::Kilograms)
        .unwrap();
    let written = parse_file(&path);
    let _ = std::fs::remove_file(&path);
    written
}

fn step_summary(session: &Session) -> Vec<StepSummary> {
    session
        .steps
        .iter()
        .map(|s| {
            (
                s.idx,
                s.ex_cat,
                s.ex_id,
                s.reps,
                s.reps_ref,
                s.weight,
                s.weight_ref,
                s.duration,
                s.duration_ref,
                s.kind.clone(),
            )
        })
        .collect()
}

type StepSummary = (
    u8,
    Option<u16>,
    Option<u16>,
    Option<u16>,
    Option<u16>,
    Option<f32>,
    Option<f32>,
    Option<u32>,
    Option<u32>,
    StepType,
);

#[test]
fn session_survives_a_fit_round_trip() {
    for name in [
        "strength_1",
        "strength_2",
        "strength_3",
        "strength_4",
        "strength_5",
        "strength_6",
        "walk_buenos_aires",
        "walk_kyoto",
        "walk_oslo",
        "indoor_biking",
    ] {
        let session = parse_fixture(name);
        let written = round_trip(&session, name);

        assert_eq!(written.date, session.date, "{name}");
        assert_eq!(written.name, session.name, "{name}");
        assert_eq!(written.device, session.device, "{name}");
        assert_eq!(written.sport, session.sport, "{name}");
        assert_eq!(written.sub_sport, session.sub_sport, "{name}");
        assert_eq!(
            written.total_elapsed_time, session.total_elapsed_time,
            "{name}"
        );
        assert_eq!(written.active_time, session.active_time, "{name}");
        assert_eq!(written.active_calories, session.active_calories, "{name}");
        assert_eq!(written.training_load, session.training_load, "{name}");
        assert_eq!(step_summary(&written), step_summary(&session), "{name}");

        let laps = |s: &Session| -> Vec<_> {
            s.laps
                .iter()
                .map(|l| (l.idx, l.start_latitude, l.start_longitude))
                .collect()
        };
        assert_eq!(laps(&written), laps(&session), "{name}");

        let data = session.data.as_ref().unwrap();
        let written_data = written.data.as_ref().unwrap();
        assert_eq!(written_data.heart_rates, data.heart_rates, "{name}");
        assert_eq!(written_data.coordinates, data.coordinates, "{name}");
        assert_eq!(written_data.speeds, data.speeds, "{name}");
        assert_eq!(written_data.distance, data.distance, "{name}");
        assert_eq!(written_data.max_hr, data.max_hr, "{name}");
    }
}

#[test]
fn session_sets_do_not_depend_on_its_current_workout() {
    let mut session = parse_fixture("strength_1");
    let expected = step_summary(&session);
    session.workout_obj = None;

    assert_eq!(step_summary(&round_trip(&session, "no_workout")), expected);
}

#[test]
fn session_without_device_uses_its_date_as_file_key() {
    let mut session = parse_fixture("strength_1");
    session.device = None;
    session.device_obj = None;

    assert_eq!(session.file_key(), session.date);
    assert!(round_trip(&session, "no_device").device.is_none());
}

#[test]
fn session_set_with_both_targets_keeps_the_reps_one() {
    let mut session = parse_fixture("strength_1");
    let idx = session
        .steps
        .iter()
        .position(|s| s.kind == StepType::Exercise)
        .unwrap();
    session.steps[idx].reps_ref = Some(12);
    session.steps[idx].duration_ref = Some(80);

    let written = round_trip(&session, "both_targets");

    assert_eq!(written.steps[idx].reps_ref, Some(12));
    assert_eq!(written.steps[idx].duration_ref, None);
}

#[test]
fn session_references_its_workout_like_a_device_recording() {
    let session = parse_fixture("strength_1");
    let messages = session
        .to_fit_messages(Languages::English, WeightUnit::Kilograms)
        .unwrap();
    let position = |num| messages.iter().position(|msg| msg.num == num).unwrap();

    assert!(position(MesgNum::SPORT) < position(MesgNum::WORKOUT));
    assert_eq!(
        position(MesgNum::TRAINING_FILE) + 1,
        position(MesgNum::WORKOUT)
    );
    assert!(position(MesgNum::WORKOUT) < position(MesgNum::RECORD));
}

#[test]
fn session_set_without_duration_keeps_it_empty() {
    let mut session = parse_fixture("strength_1");
    session.steps[0].duration = None;

    let written = round_trip(&session, "no_duration");

    assert_eq!(written.steps[0].duration, None);
    assert!(written.steps[1].duration.is_some());
}

#[test]
fn step_time_is_in_milliseconds_only_inside_the_fit_file() {
    let workout = Workout {
        name: "Push".to_string(),
        enabled: true,
        notes: None,
        connect_id: None,
        steps: vec![WorkoutStep::rest("Push", 0, Some(90), None)],
    };
    let messages = workout
        .to_fit_messages(Languages::English, WeightUnit::Kilograms)
        .unwrap();
    let step = messages
        .iter()
        .find(|msg| msg.num == MesgNum::WORKOUT_STEP)
        .unwrap();

    assert_eq!(mesgdef::WorkoutStep::from(step).duration_value, 90_000);
}

#[test]
fn session_duration_target_stays_in_seconds_after_a_round_trip() {
    let mut session = parse_fixture("strength_1");
    let idx = session
        .steps
        .iter()
        .position(|s| s.kind == StepType::Rest)
        .unwrap();
    session.steps[idx].duration_ref = Some(90);

    assert_eq!(
        round_trip(&session, "duration_ref").steps[idx].duration_ref,
        Some(90)
    );
}

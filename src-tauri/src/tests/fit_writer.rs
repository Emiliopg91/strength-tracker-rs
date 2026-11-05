use rustyfit::profile::{
    mesgdef,
    typedef::{Intensity, MesgNum},
};

use crate::{
    dao::{
        settings::WeightUnit,
        workout::Workout,
        workout_step::{StepType, WorkoutStep},
    },
    fit::writer::ToFitMessages,
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

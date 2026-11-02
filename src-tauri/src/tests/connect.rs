use serde_json::{Value, json};

use crate::dao::{
    exercise::Exercise,
    exercise_category::ExerciseCategory,
    settings::WeightUnit,
    workout::Workout,
    workout_step::{StepType, WorkoutStep},
};
use crate::dto::connect::{self, ConnectWorkout};

fn step(idx: u16, kind: StepType) -> WorkoutStep {
    WorkoutStep {
        workout: "Push".to_string(),
        idx,
        kind,
        ex_cat: None,
        ex_id: None,
        reps: None,
        weight: None,
        time: None,
        begin_idx: None,
        notes: None,
        exercise: None,
    }
}

fn exercise_step(idx: u16) -> WorkoutStep {
    WorkoutStep {
        ex_cat: Some(0),
        ex_id: Some(1),
        exercise: Some(Exercise {
            category: 0,
            id: 1,
            unilateral: false,
            progression: true,
            reps: "8-10-12".to_string(),
            weight: 2.5,
            notes: None,
            name: "BARBELL_BENCH_PRESS".to_string(),
            exercise_category: Some(ExerciseCategory {
                id: 0,
                name: "BENCH_PRESS".to_string(),
            }),
            muscles: Vec::new(),
        }),
        ..step(idx, StepType::Exercise)
    }
}

fn reps(idx: u16, reps: u16, weight: Option<f32>) -> WorkoutStep {
    WorkoutStep {
        reps: Some(Some(reps)),
        weight,
        ..exercise_step(idx)
    }
}

fn rest(idx: u16, time_ms: Option<u32>) -> WorkoutStep {
    WorkoutStep {
        time: Some(time_ms),
        ..step(idx, StepType::Rest)
    }
}

fn repeat(idx: u16, begin_idx: u16, iterations: u16) -> WorkoutStep {
    WorkoutStep {
        reps: Some(Some(iterations)),
        begin_idx: Some(begin_idx),
        ..step(idx, StepType::Repeat)
    }
}

fn steps_json(steps: &[WorkoutStep]) -> Value {
    steps_json_in(steps, WeightUnit::Kilograms)
}

fn steps_json_in(steps: &[WorkoutStep], weight_unit: WeightUnit) -> Value {
    serde_json::to_value(connect::WorkoutStep::from_workout(steps, weight_unit)).unwrap()
}

/// `(stepOrder, childStepId)` of every step, depth-first, as Connect expects them
fn orders(steps: &Value) -> Vec<(u64, Option<u64>)> {
    let mut res = Vec::new();
    for step in steps.as_array().unwrap() {
        res.push((
            step["stepOrder"].as_u64().unwrap(),
            step["childStepId"].as_u64(),
        ));
        if step["type"] == "RepeatGroupDTO" {
            res.extend(orders(&step["workoutSteps"]));
        }
    }
    res
}

#[test]
fn exercise_with_reps_and_weight() {
    let mut step = reps(0, 10, Some(60.0));
    step.notes = Some("Slow eccentric".to_string());
    let json = steps_json(&[step]);

    assert_eq!(
        json[0],
        json!({
            "type": "ExecutableStepDTO",
            "stepOrder": 1,
            "stepType": { "stepTypeId": 3, "stepTypeKey": "interval" },
            "endCondition": { "conditionTypeId": 10, "conditionTypeKey": "reps" },
            "endConditionValue": 10.0,
            "category": "BENCH_PRESS",
            "exerciseName": "BARBELL_BENCH_PRESS",
            "weightValue": 60.0,
            "weightUnit": { "unitId": 8, "unitKey": "kilogram", "factor": 1000.0 },
            "description": "Slow eccentric",
        })
    );
}

#[test]
fn exercise_without_weight_omits_weight_fields() {
    let json = steps_json(&[reps(0, 12, None)]);

    assert!(json[0].get("weightValue").is_none());
    assert!(json[0].get("weightUnit").is_none());
    assert!(json[0].get("description").is_none());
}

#[test]
fn timed_exercise_ends_on_time_in_seconds() {
    let step = WorkoutStep {
        time: Some(Some(45_000)),
        ..exercise_step(0)
    };
    let json = steps_json(&[step]);

    assert_eq!(json[0]["endCondition"]["conditionTypeKey"], "time");
    assert_eq!(json[0]["endConditionValue"], 45.0);
}

#[test]
fn time_wins_over_reps_like_the_fit_writer() {
    let step = WorkoutStep {
        time: Some(Some(30_000)),
        ..reps(0, 10, None)
    };
    let json = steps_json(&[step]);

    assert_eq!(json[0]["endCondition"]["conditionTypeKey"], "time");
    assert_eq!(json[0]["endConditionValue"], 30.0);
}

#[test]
fn weight_is_converted_to_pounds() {
    // What the frontend stores for 135 lb: round(135 / 2.20462, 1)
    let json = steps_json_in(&[reps(0, 10, Some(61.2))], WeightUnit::Pounds);

    assert_eq!(json[0]["weightValue"], 134.9);
    assert_eq!(
        json[0]["weightUnit"],
        json!({ "unitId": 9, "unitKey": "pound", "factor": 453.59237 })
    );
}

#[test]
fn weight_in_kilograms_is_sent_as_is() {
    let json = steps_json(&[reps(0, 10, Some(61.2))]);

    assert_eq!(json[0]["weightValue"], 61.2);
}

#[test]
fn open_exercise_ends_on_lap_button() {
    let json = steps_json(&[exercise_step(0)]);

    assert_eq!(json[0]["endCondition"]["conditionTypeKey"], "lap.button");
    assert!(json[0].get("endConditionValue").is_none());
}

#[test]
fn rest_with_and_without_time() {
    let json = steps_json(&[rest(0, Some(90_000)), rest(1, None)]);

    assert_eq!(json[0]["stepType"]["stepTypeKey"], "rest");
    assert_eq!(json[0]["endCondition"]["conditionTypeKey"], "time");
    assert_eq!(json[0]["endConditionValue"], 90.0);
    assert!(json[0].get("category").is_none());
    assert!(json[0].get("exerciseName").is_none());

    assert_eq!(json[1]["endCondition"]["conditionTypeKey"], "lap.button");
    assert!(json[1].get("endConditionValue").is_none());
}

#[test]
fn flat_steps_are_numbered_in_order_without_parent() {
    let json = steps_json(&[
        reps(0, 10, Some(60.0)),
        rest(1, Some(90_000)),
        reps(2, 8, Some(65.0)),
    ]);

    assert_eq!(orders(&json), vec![(1, None), (2, None), (3, None)]);
    assert_eq!(json[2]["weightValue"], 65.0);
}

#[test]
fn repeat_group_wraps_its_range() {
    let json = steps_json(&[
        reps(0, 10, Some(60.0)),
        rest(1, Some(90_000)),
        repeat(2, 0, 3),
    ]);

    assert_eq!(json.as_array().unwrap().len(), 1);
    let group = &json[0];
    assert_eq!(group["type"], "RepeatGroupDTO");
    assert_eq!(group["stepType"]["stepTypeKey"], "repeat");
    assert_eq!(group["endCondition"]["conditionTypeKey"], "iterations");
    assert_eq!(group["endConditionValue"], 3.0);
    assert_eq!(group["numberOfIterations"], 3);
    assert_eq!(group["smartRepeat"], false);

    let children = group["workoutSteps"].as_array().unwrap();
    assert_eq!(children.len(), 2);
    assert_eq!(children[0]["stepType"]["stepTypeKey"], "interval");
    assert_eq!(children[1]["stepType"]["stepTypeKey"], "rest");

    // The group goes before its children, which point to its own childStepId
    assert_eq!(
        orders(&json),
        vec![(1, Some(1)), (2, Some(1)), (3, Some(1))]
    );
}

#[test]
fn steps_around_a_repeat_keep_their_place() {
    let json = steps_json(&[
        reps(0, 15, None),
        reps(1, 10, Some(60.0)),
        rest(2, Some(90_000)),
        repeat(3, 1, 4),
        rest(4, None),
    ]);

    let top = json.as_array().unwrap();
    assert_eq!(top.len(), 3);
    assert_eq!(top[0]["type"], "ExecutableStepDTO");
    assert_eq!(top[1]["type"], "RepeatGroupDTO");
    assert_eq!(top[2]["stepType"]["stepTypeKey"], "rest");
    assert_eq!(top[1]["workoutSteps"].as_array().unwrap().len(), 2);

    assert_eq!(
        orders(&json),
        vec![
            (1, None),
            (2, Some(1)),
            (3, Some(1)),
            (4, Some(1)),
            (5, None)
        ]
    );
}

#[test]
fn sibling_repeats_get_their_own_child_step_id() {
    let json = steps_json(&[
        reps(0, 10, Some(60.0)),
        repeat(1, 0, 3),
        reps(2, 12, Some(20.0)),
        repeat(3, 2, 2),
    ]);

    assert_eq!(
        orders(&json),
        vec![(1, Some(1)), (2, Some(1)), (3, Some(2)), (4, Some(2))]
    );
}

#[test]
fn nested_repeats_are_numbered_depth_first() {
    // 0 | outer x3 [ inner x2 [ 1, 2 ], 4 ]
    let json = steps_json(&[
        reps(0, 15, None),
        reps(1, 10, Some(60.0)),
        rest(2, Some(60_000)),
        repeat(3, 1, 2),
        reps(4, 12, Some(20.0)),
        repeat(5, 1, 3),
    ]);

    let top = json.as_array().unwrap();
    assert_eq!(top.len(), 2);
    let outer = &top[1];
    assert_eq!(outer["numberOfIterations"], 3);
    let outer_children = outer["workoutSteps"].as_array().unwrap();
    assert_eq!(outer_children.len(), 2);
    assert_eq!(outer_children[0]["type"], "RepeatGroupDTO");
    assert_eq!(outer_children[0]["numberOfIterations"], 2);
    assert_eq!(
        outer_children[0]["workoutSteps"].as_array().unwrap().len(),
        2
    );

    assert_eq!(
        orders(&json),
        vec![
            (1, None),
            (2, Some(1)),
            (3, Some(2)),
            (4, Some(2)),
            (5, Some(2)),
            (6, Some(1)),
        ]
    );
}

#[test]
fn workout_envelope() {
    let workout = Workout {
        name: "Push".to_string(),
        enabled: true,
        notes: Some("Chest day".to_string()),
        connect_id: Some("123456".to_string()),
        steps: vec![reps(0, 10, Some(60.0))],
    };
    let json = serde_json::to_value(ConnectWorkout::from(&workout, WeightUnit::Kilograms)).unwrap();

    let strength = json!({ "sportTypeId": 5, "sportTypeKey": "strength_training" });
    assert_eq!(json["workoutId"], 123456);
    assert_eq!(json["workoutName"], "Push");
    assert_eq!(json["description"], "Chest day");
    assert_eq!(json["sportType"], strength);
    let segments = json["workoutSegments"].as_array().unwrap();
    assert_eq!(segments.len(), 1);
    assert_eq!(segments[0]["segmentOrder"], 1);
    assert_eq!(segments[0]["sportType"], strength);
    assert_eq!(segments[0]["workoutSteps"].as_array().unwrap().len(), 1);
}

#[test]
fn new_workout_omits_id_and_description() {
    let workout = Workout {
        name: "Pull".to_string(),
        enabled: true,
        notes: None,
        connect_id: None,
        steps: Vec::new(),
    };
    let json = serde_json::to_value(ConnectWorkout::from(&workout, WeightUnit::Kilograms)).unwrap();

    assert!(json.get("workoutId").is_none());
    assert!(json.get("description").is_none());
}

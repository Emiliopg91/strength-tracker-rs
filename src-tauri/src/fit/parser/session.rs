use crate::{
    dao::{
        device::Device,
        exercise::Exercise,
        session::Session,
        session_data::SessionData,
        session_step::SessionStep,
        sport::Sport,
        sub_sport::SubSport,
        workout_step::{StepType, WorkoutStep},
    },
    fit::parser::{
        FitParser,
        errors::{self, ParseFitFileError},
        workout::WorkoutAccumulator,
    },
};
use rustyfit::{
    DecoderEvent, StreamingIterator,
    profile::{
        mesgdef,
        typedef::{DeviceIndex, MesgNum},
    },
};

impl TryFrom<FitParser> for Session {
    type Error = errors::ParseFitFileError;
    fn try_from(mut value: FitParser) -> Result<Self, Self::Error> {
        let mut session_data = SessionAccumulator::new();
        let mut records = RecordAccumulator::new();
        let mut exercises = Vec::new();
        let mut sets_data = Vec::new();
        let mut laps = Vec::new();
        let mut workout = WorkoutAccumulator::default();
        let mut max_heart_rate = None;
        let mut device = None;

        let path_string = value.borrow_name().clone();

        value.with_stream_mut(|stream| -> errors::Result<()> {
            while let Some(event) = stream.next() {
                let event = event.map_err(|e| {
                    ParseFitFileError::FileReading(path_string.clone(), Box::new(e))
                })?;
                if let DecoderEvent::Message(msg) = event {
                    match msg.num {
                        MesgNum::WORKOUT => {
                            let workout_obj = mesgdef::Workout::from(msg);
                            // A non-strength workout only leaves the session without workout_obj
                            let _ = workout.set_workout(&workout_obj);
                            session_data.set_workout(workout_obj);
                        }
                        MesgNum::DEVICE_INFO => {
                            let device_info_obj = mesgdef::DeviceInfo::from(msg);
                            // The other indices are paired sensors, usually without serial
                            if device_info_obj.device_index == DeviceIndex::CREATOR
                                && device_info_obj.serial_number != 0
                            {
                                device = Some(Device::serial_from_unit_id(
                                    device_info_obj.serial_number as u64,
                                ));
                            }
                        }
                        MesgNum::SESSION => {
                            let session_obj = mesgdef::Session::from(msg);
                            records.distance = session_obj.total_distance_scaled();
                            session_data.set_session(session_obj)?;
                        }
                        MesgNum::WORKOUT_STEP => {
                            let step_obj = mesgdef::WorkoutStep::from(msg);
                            workout.push_step(&step_obj);
                            handle_step_message(&step_obj, &mut exercises)
                        }
                        MesgNum::RECORD => {
                            let record_obj = mesgdef::Record::from(msg);
                            records.push(record_obj);
                        }
                        MesgNum::SET => {
                            let set_obj = mesgdef::Set::from(msg);
                            handle_set_message(set_obj, &mut sets_data);
                        }
                        MesgNum::LAP => {
                            let lap_obj = mesgdef::Lap::from(msg);
                            handle_lap_message(lap_obj, &mut laps);
                        }
                        MesgNum::TIME_IN_ZONE if max_heart_rate.is_none() => {
                            let zones_obj = mesgdef::TimeInZone::from(msg);
                            max_heart_rate = Some(zones_obj.max_heart_rate);
                        }
                        _ => {}
                    }
                }
            }
            Ok(())
        })?;

        let workout_obj = workout.build().ok();

        let mut step_idx = 0;
        let mut steps: Vec<_> = sets_data
            .into_iter()
            .map(|(idx, reps, weight, duration)| {
                let wkt_step = workout_obj.as_ref().and_then(|w| w.steps.get(idx));
                let duration_ref = wkt_step.and_then(|s| s.time.flatten());
                let has_exercise = matches!(exercises.get(idx), Some(Some(_)));
                let kind = set_kind(wkt_step, has_exercise);

                let res = if let Some(exercise) = exercises.get(idx)
                    && let Some(exercise) = exercise
                {
                    SessionStep {
                        session: session_data.timestamp,
                        idx: step_idx,
                        ex_cat: Some(exercise.category),
                        ex_id: Some(exercise.id),
                        reps: Some(reps),
                        reps_ref: wkt_step.and_then(|s| s.reps.flatten()),
                        weight: Some(weight),
                        // Workout steps without a target weight store it as 0
                        weight_ref: wkt_step.and_then(|s| s.weight).filter(|w| *w > 0_f32),
                        pr: false,
                        exercise: Some(exercise.clone()),
                        duration,
                        duration_ref,
                        kind,
                    }
                } else {
                    SessionStep {
                        session: session_data.timestamp,
                        idx: step_idx,
                        ex_cat: None,
                        ex_id: None,
                        // Other kinds without exercise, like warm-ups, are still done sets
                        reps: (kind != StepType::Rest).then_some(0),
                        reps_ref: None,
                        weight: (kind != StepType::Rest).then_some(0_f32),
                        weight_ref: None,
                        pr: false,
                        exercise: None,
                        duration,
                        duration_ref,
                        kind,
                    }
                };
                step_idx += 1;
                res
            })
            .collect();

        // Drop trailing rest steps while the last two are both rests
        while let [.., a, b] = steps.as_slice()
            && a.kind == StepType::Rest
            && b.kind == StepType::Rest
        {
            steps.pop();
        }

        let mut lap_idx = 0;
        laps = laps
            .into_iter()
            .skip(1)
            .map(|mut l| {
                l.session = session_data.timestamp;
                l.idx = lap_idx;
                lap_idx += 1;
                l
            })
            .collect();

        records.timestamp = session_data.timestamp;
        let mut additional_data: Option<SessionData> = records.into();
        if let Some(ad) = additional_data.as_mut()
            && ad.heart_rates.is_some()
        {
            ad.max_hr = max_heart_rate;
        }

        Ok(Session {
            date: session_data.timestamp,
            name: session_data.workout.clone().unwrap_or_default(),
            workout_obj,
            workout: session_data.workout,
            total_elapsed_time: session_data.total_elapsed_time,
            active_time: session_data.active_time,
            active_calories: session_data.total_calories - session_data.metabolic_calories,
            steps,
            training_load: session_data.training_load,
            sport: session_data.sub_sport_obj.sport,
            sub_sport: session_data.sub_sport_obj.id,
            sub_sport_obj: Some(session_data.sub_sport_obj),
            laps,
            device,
            device_obj: None,
            data: additional_data,
            connect_id: None,
        })
    }
}

/// Sets of steps without exercise are rests, unless their workout step says otherwise.
fn set_kind(wkt_step: Option<&WorkoutStep>, has_exercise: bool) -> StepType {
    match wkt_step.map(|s| &s.kind) {
        Some(
            kind @ (StepType::Warmup | StepType::Cooldown | StepType::Recovery | StepType::Other),
        ) => kind.clone(),
        _ if has_exercise => StepType::Exercise,
        _ => StepType::Rest,
    }
}

fn handle_lap_message(msg: mesgdef::Lap, laps: &mut Vec<crate::dao::session_lap::SessionLap>) {
    let start_latitude = (msg.start_position_lat != i32::MAX).then_some(msg.start_position_lat);
    let start_longitude = (msg.start_position_long != i32::MAX).then_some(msg.start_position_long);

    laps.push(crate::dao::session_lap::SessionLap {
        session: 0,
        idx: 0,
        start_latitude,
        start_longitude,
    });
}

fn handle_set_message(msg: mesgdef::Set, sets_data: &mut Vec<(usize, u16, f32, Option<u32>)>) {
    let ex_idx = msg.wkt_step_index.0 as usize;
    if msg.repetitions != u16::MAX
        && msg.wkt_step_index.0 != u16::MAX
        && let Some(weight) = msg.weight_scaled()
    {
        sets_data.push((
            ex_idx,
            msg.repetitions,
            weight as f32,
            msg.duration_scaled().map(|d| d as u32),
        ));
    } else {
        sets_data.push((ex_idx, 0, 0_f32, msg.duration_scaled().map(|d| d as u32)));
    }
}

fn handle_step_message(msg: &mesgdef::WorkoutStep, exercises: &mut Vec<Option<Exercise>>) {
    // An invalid name is the generic exercise of the category, stored under that same id
    let exercise = (msg.exercise_category.0 != u16::MAX).then(|| Exercise {
        category: msg.exercise_category.0,
        id: msg.exercise_name,
        unilateral: false,
        exercise_category: None,
        progression: false,
        reps: String::new(),
        weight: 0_f32,
        muscles: vec![],
        notes: None,
        name: String::new(),
    });
    exercises.push(exercise);
}

/// Accumulates the session-level scalar fields (from the `session`/`workout` FIT messages) in
/// place as they stream in, instead of threading them through a tuple return + destructuring
/// assignment back in `parse`.
struct SessionAccumulator {
    workout: Option<String>,
    timestamp: u32,
    sub_sport_obj: SubSport,
    total_elapsed_time: u32,
    active_time: u32,
    training_load: u16,
    total_calories: u16,
    metabolic_calories: u16,
}

impl SessionAccumulator {
    fn new() -> Self {
        Self {
            workout: None,
            timestamp: 0,
            sub_sport_obj: SubSport {
                id: 0,
                sport: 0,
                sport_obj: None,
            },
            total_elapsed_time: 0,
            active_time: 0,
            training_load: 0,
            total_calories: 0,
            metabolic_calories: 0,
        }
    }

    fn set_workout(&mut self, msg: mesgdef::Workout) {
        self.workout = Some(msg.wkt_name.to_string());
    }

    fn set_session(&mut self, msg: mesgdef::Session) -> errors::Result<()> {
        self.timestamp = msg
            .timestamp
            .unix_timestamp()
            .ok_or_else(|| ParseFitFileError::MissingField("timestamp".to_string()))?
            as u32;

        let sport = msg.sport.0;
        if sport == u8::MAX {
            return Err(ParseFitFileError::MissingField("sport".to_string()));
        }
        let sub_sport = msg.sub_sport.0;
        if sub_sport == u8::MAX {
            return Err(ParseFitFileError::MissingField("sub_sport".to_string()));
        }
        self.sub_sport_obj = SubSport {
            id: sub_sport,
            sport,
            sport_obj: Some(Sport { id: sport }),
        };

        self.total_elapsed_time = msg
            .total_elapsed_time_scaled()
            .map(|v| v.round() as u32)
            .ok_or_else(|| ParseFitFileError::MissingField("total_elapsed_time".to_string()))?;

        self.active_time = msg.active_time_scaled().map_or(0_u32, |v| v.round() as u32);

        self.training_load = msg
            .training_load_peak_scaled()
            .map(|e| e.round() as u16)
            .ok_or_else(|| ParseFitFileError::MissingField("training_load_peak".to_string()))?;

        self.total_calories = (msg.total_calories != u16::MAX)
            .then_some(msg.total_calories)
            .ok_or_else(|| ParseFitFileError::MissingField("total_calories".to_string()))?;

        self.metabolic_calories = (msg.metabolic_calories != u16::MAX)
            .then_some(msg.metabolic_calories)
            .ok_or_else(|| ParseFitFileError::MissingField("metabolic_calories".to_string()))?;

        Ok(())
    }
}

/// Accumulates per-record time series (HR, cadence, GPS, power, speed, respiration) across a
/// single streaming pass, resolving each field to its final scalar (with sentinel fallback, or
/// forward-filled for GPS).
#[derive(Default)]
struct RecordAccumulator {
    timestamp: u32,
    hrs: Vec<u8>,
    any_hr: bool,
    coords: Vec<(i32, i32)>,
    any_coord: bool,
    last_coord: (i32, i32),
    speeds: Vec<f64>,
    any_speed: bool,
    distance: Option<f64>,
}

impl RecordAccumulator {
    fn new() -> Self {
        Self {
            last_coord: (SessionData::INVALID_POSITION, SessionData::INVALID_POSITION),
            ..Default::default()
        }
    }

    fn push(&mut self, msg: mesgdef::Record) {
        self.any_hr |= msg.heart_rate != SessionData::INVALID_HEAR_RATE;
        self.hrs.push(msg.heart_rate);

        if msg.position_lat != SessionData::INVALID_POSITION
            && msg.position_long != SessionData::INVALID_POSITION
        {
            self.last_coord = (msg.position_lat, msg.position_long);
            self.any_coord = true;
        }
        self.coords.push(self.last_coord);

        self.speeds
            .push(match msg.enhanced_speed_scaled().or(msg.speed_scaled()) {
                Some(v) => {
                    self.any_speed = true;
                    v
                }
                None => SessionData::INVALID_SPEED,
            });
    }
}

impl From<RecordAccumulator> for Option<SessionData> {
    fn from(value: RecordAccumulator) -> Self {
        let coords = value.any_coord.then_some(value.coords);
        let hrs = value.any_hr.then_some(value.hrs);
        let speeds = value.any_speed.then_some(value.speeds);

        if hrs.is_some() || coords.is_some() || speeds.is_some() || value.distance.is_some() {
            Some(SessionData {
                session: value.timestamp,
                heart_rates: hrs,
                coordinates: coords.map(|coords| SessionData::build_coordinates_blob(&coords)),
                speeds: speeds.map(|speeds| SessionData::build_speeds_blob(&speeds)),
                distance: value.distance,
                notes: None,
                max_hr: None,
            })
        } else {
            None
        }
    }
}

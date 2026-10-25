use crate::{
    dao::{
        additional_data::AdditionalData, exercise::Exercise, session::Session, set::Set,
        sport::Sport, sub_sport::SubSport,
    },
    fit::parser::{
        FitParser,
        errors::{self, ParseFitFileError},
        workout::WorkoutAccumulator,
    },
};
use rustyfit::{
    DecoderEvent, StreamingIterator,
    profile::{mesgdef, typedef::MesgNum},
};

impl TryFrom<FitParser> for Session {
    type Error = errors::ParseFitFileError;
    fn try_from(mut value: FitParser) -> Result<Self, Self::Error> {
        let mut session_data = SessionAccumulator::new();
        let mut records = RecordAccumulator::new();
        let mut exercises = Vec::new();
        let mut series_data = Vec::new();
        let mut laps = Vec::new();
        let mut workout = WorkoutAccumulator::default();
        let mut max_heart_rate = None;

        let path_string = value.borrow_path().display().to_string();

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
                            handle_set_message(set_obj, &mut series_data);
                        }
                        MesgNum::LAP => {
                            let lap_obj = mesgdef::Lap::from(msg);
                            handle_lap_message(lap_obj, &mut laps)?;
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

        let mut serie_idx = 0;
        let mut series: Vec<_> = series_data
            .into_iter()
            .map(|(idx, reps, weight, duration)| {
                let res = if let Some(exercise) = exercises.get(idx)
                    && let Some(exercise) = exercise
                {
                    Set {
                        session: session_data.timestamp,
                        idx: serie_idx,
                        ex_cat: exercise.category,
                        ex_id: exercise.id,
                        reps,
                        weight,
                        pr: false,
                        exercise: Some(exercise.clone()),
                        duration,
                    }
                } else {
                    Set {
                        session: session_data.timestamp,
                        idx: serie_idx,
                        ex_cat: u16::MAX,
                        ex_id: 0,
                        reps: 0,
                        weight: 0_f32,
                        pr: false,
                        exercise: None,
                        duration,
                    }
                };
                serie_idx += 1;
                res
            })
            .collect();

        // Drop trailing rest sets while the last two are both rests
        while let [.., a, b] = series.as_slice()
            && a.ex_cat == u16::MAX
            && b.ex_cat == u16::MAX
        {
            series.pop();
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
        let mut additional_data: Option<AdditionalData> = records.into();
        if let Some(ad) = additional_data.as_mut()
            && ad.heart_rates.is_some()
        {
            ad.max_hr = max_heart_rate;
        }

        Ok(Session {
            date: session_data.timestamp,
            name: session_data.workout.clone().unwrap_or_default(),
            workout_obj: workout.build().ok(),
            workout: session_data.workout,
            total_elapsed_time: session_data.total_elapsed_time,
            active_time: session_data.active_time,
            active_calories: session_data.total_calories - session_data.metabolic_calories,
            sets: series,
            training_load: session_data.training_load,
            sport: session_data.sub_sport_obj.sport,
            sub_sport: session_data.sub_sport_obj.id,
            sub_sport_obj: Some(session_data.sub_sport_obj),
            laps,
            device: None,
            device_obj: None,
            additional_data,
        })
    }
}

fn handle_lap_message(
    msg: mesgdef::Lap,
    laps: &mut Vec<crate::dao::lap::Lap>,
) -> errors::Result<()> {
    let start_latitude = if msg.start_position_lat != i32::MAX {
        Some(msg.start_position_lat)
    } else {
        None
    };
    let start_longitude = if msg.start_position_long != i32::MAX {
        Some(msg.start_position_long)
    } else {
        None
    };

    laps.push(crate::dao::lap::Lap {
        session: 0,
        idx: 0,
        start_latitude,
        start_longitude,
    });

    Ok(())
}

fn handle_set_message(msg: mesgdef::Set, series_data: &mut Vec<(usize, u16, f32, u32)>) {
    let ex_idx = msg.wkt_step_index.0 as usize;
    if msg.repetitions != u16::MAX
        && msg.wkt_step_index.0 != u16::MAX
        && let Some(weight) = msg.weight_scaled()
    {
        series_data.push((
            ex_idx,
            msg.repetitions,
            weight as f32,
            msg.duration_scaled().unwrap_or(0_f64) as u32,
        ));
    } else {
        series_data.push((
            ex_idx,
            0,
            0_f32,
            msg.duration_scaled().unwrap_or(0_f64) as u32,
        ));
    }
}

fn handle_step_message(msg: &mesgdef::WorkoutStep, exercises: &mut Vec<Option<Exercise>>) {
    if msg.exercise_category.0 != u16::MAX {
        let ex_cat = msg.exercise_category.0;
        let ex_id = if msg.exercise_name == u16::MAX {
            1
        } else {
            msg.exercise_name
        };

        exercises.push(Some(Exercise {
            category: ex_cat,
            id: ex_id,
            unilateral: false,
            exercise_category: None,
            progression: false,
            reps: "".to_string(),
        }))
    } else {
        exercises.push(None)
    }
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

        self.sub_sport_obj = {
            let sport_val = msg.sport;
            let sub_sport_val = msg.sub_sport;
            if sport_val.0 != u8::MAX {
                if sub_sport_val.0 != u8::MAX {
                    Ok(SubSport {
                        id: sub_sport_val.0,
                        sport: sport_val.0,
                        sport_obj: Some(Sport { id: sport_val.0 }),
                    })
                } else {
                    Err(ParseFitFileError::MissingField("sub_sport".to_string()))
                }
            } else {
                Err(ParseFitFileError::MissingField("sport".to_string()))
            }
        }?;

        self.total_elapsed_time = msg
            .total_elapsed_time_scaled()
            .map(|v| v.round() as u32)
            .ok_or_else(|| ParseFitFileError::MissingField("total_elapsed_time".to_string()))?;

        self.active_time = msg.active_time_scaled().map_or(0_u32, |v| v.round() as u32);

        self.training_load = msg
            .training_load_peak_scaled()
            .map(|e| e.round() as u16)
            .ok_or_else(|| ParseFitFileError::MissingField("training_load_peak".to_string()))?;

        self.total_calories = if msg.total_calories == u16::MAX {
            Err(ParseFitFileError::MissingField(
                "total_calories".to_string(),
            ))
        } else {
            Ok(msg.total_calories)
        }?;

        self.metabolic_calories = if msg.metabolic_calories == u16::MAX {
            Err(ParseFitFileError::MissingField(
                "metabolic_calories".to_string(),
            ))
        } else {
            Ok(msg.metabolic_calories)
        }?;

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
    altitudes: Vec<f64>,
    any_altitude: bool,
    distance: Option<f64>,
}

impl RecordAccumulator {
    fn new() -> Self {
        Self {
            last_coord: (
                AdditionalData::INVALID_POSITION,
                AdditionalData::INVALID_POSITION,
            ),
            ..Default::default()
        }
    }

    fn push(&mut self, msg: mesgdef::Record) {
        self.any_hr |= msg.heart_rate != AdditionalData::INVALID_HEAR_RATE;
        self.hrs.push(msg.heart_rate);

        if msg.position_lat != AdditionalData::INVALID_POSITION
            && msg.position_long != AdditionalData::INVALID_POSITION
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
                None => AdditionalData::INVALID_SPEED,
            });

        self.altitudes.push(
            match msg.enhanced_altitude_scaled().or(msg.altitude_scaled()) {
                Some(v) => {
                    self.any_altitude = true;
                    v
                }
                None => AdditionalData::INVALID_ALTITUDE,
            },
        );
    }
}

impl From<RecordAccumulator> for Option<AdditionalData> {
    fn from(value: RecordAccumulator) -> Self {
        let coords = value.any_coord.then_some(value.coords);
        let hrs = value.any_hr.then_some(value.hrs);
        let speeds = value.any_speed.then_some(value.speeds);
        let altitudes = value.any_altitude.then_some(value.altitudes);

        if hrs.is_some() || coords.is_some() || speeds.is_some() || value.distance.is_some() {
            Some(AdditionalData {
                session: value.timestamp,
                heart_rates: hrs,
                coordinates: coords.map(|coords| AdditionalData::build_coordinates_blob(&coords)),
                speeds: speeds.map(|speeds| AdditionalData::build_speeds_blob(&speeds)),
                distance: value.distance,
                altitudes: altitudes
                    .map(|altitudes| AdditionalData::build_altitudes_blob(&altitudes)),
                notes: None,
                max_hr: None,
            })
        } else {
            None
        }
    }
}

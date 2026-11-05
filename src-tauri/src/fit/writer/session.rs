use chrono::{Local, TimeZone};
use rustyfit::{
    profile::{
        mesgdef::{
            Activity, DeviceInfo, Event, Lap, Record, Session as FitSession, Set,
            Sport as FitSport, TimeInZone, TrainingFile, Workout as FitWorkout,
        },
        typedef::{
            self, DateTime, DeviceIndex, EventType, ExerciseCategory, FitBaseUnit, GarminProduct,
            LapTrigger, LocalDateTime, Manufacturer, MesgNum, MessageIndex, SessionTrigger,
            SetType, Sport, SubSport,
        },
    },
    proto::Message,
};

use crate::{
    dao::{
        additional_data::AdditionalData,
        device::Device,
        session::Session,
        session_step::SessionStep,
        settings::WeightUnit,
        workout::Workout,
        workout_step::{StepType, WorkoutStep},
    },
    fit::writer::ToFitMessages,
    utils::translations::{Languages, translate},
};

fn datetime(secs: u32) -> DateTime {
    DateTime::from_unix_timestamp(secs as i64)
}

/// Point `part / total` of the way through `elapsed` seconds after `start`.
fn time_at(start: u32, elapsed: u32, part: usize, total: usize) -> u32 {
    start + (part as u64 * elapsed as u64 / total as u64) as u32
}

fn timer_event(timestamp: u32, event_type: EventType) -> Message {
    let mut event = Event::new();
    event.timestamp = datetime(timestamp);
    event.event = typedef::Event::TIMER;
    event.event_type = event_type;
    event.into()
}

/// Sessions don't keep record times, so samples are spread evenly as the charts assume.
fn record_messages(data: &AdditionalData, start: u32, elapsed: u32) -> Vec<Message> {
    let hrs = data.get_heart_rates().unwrap_or_default();
    let coords = data.get_coordinates_semicircle().unwrap_or_default();
    let speeds = data.get_speeds().unwrap_or_default();
    let count = hrs.len().max(coords.len()).max(speeds.len());

    (0..count)
        .map(|i| {
            let mut record = Record::new();
            record.timestamp = datetime(time_at(start, elapsed, i, count));
            if let Some(Some(hr)) = hrs.get(i) {
                record.heart_rate = *hr;
            }
            if let Some(Some((lat, long))) = coords.get(i) {
                record.position_lat = *lat;
                record.position_long = *long;
            }
            if let Some(Some(speed)) = speeds.get(i) {
                // The scaled setter truncates, which would shift the value on every round trip
                record.enhanced_speed = (speed * 1000.0).round() as u32;
            }
            record.into()
        })
        .collect()
}

/// One step per set, as the sets don't keep which step of the original workout they came from
/// (and that workout may have changed since). The parser reads the targets back from these steps.
fn workout_step(name: &str, idx: u16, step: &SessionStep) -> WorkoutStep {
    let mut wkt_step = WorkoutStep::new(name, idx, step.kind.clone());
    wkt_step.ex_cat = step.ex_cat;
    wkt_step.ex_id = step.ex_id;
    wkt_step.weight = step.weight_ref;
    // A step has a single duration, and stored sets may have both targets: reps are the real one
    match step.reps_ref.filter(|reps| *reps > 0) {
        Some(reps) => wkt_step.reps = Some(Some(reps)),
        None => wkt_step.time = Some(step.duration_ref),
    }
    wkt_step
}

fn sport_name(session: &Session, lang: Languages) -> String {
    translate(
        &format!("sport_{}_{}", session.sport, session.sub_sport),
        lang,
    )
}

fn sport_message(session: &Session, lang: Languages) -> Message {
    let mut sport = FitSport::new();
    sport.sport = Sport(session.sport);
    sport.sub_sport = SubSport(session.sub_sport);
    sport.name = sport_name(session, lang).into();
    sport.into()
}

/// Reference to the workout file followed in the activity, which devices write next to the workout.
fn training_file_message(session: &Session, workout: &Workout) -> Message {
    let mut training_file = TrainingFile::new();
    training_file.timestamp = datetime(session.date);
    training_file.r#type = typedef::File::WORKOUT;
    training_file.time_created = datetime(session.date);
    training_file.serial_number = workout.file_key();
    if workout.connect_id.is_some() {
        training_file.manufacturer = Manufacturer::GARMIN;
        training_file.product = GarminProduct::CONNECT.0;
    } else {
        training_file.manufacturer = Manufacturer::DEVELOPMENT;
    }
    training_file.into()
}

fn set_message(idx: u16, step: &SessionStep, start: u32, weight_unit: WeightUnit) -> Message {
    let mut set = Set::new();
    set.message_index = MessageIndex(idx);
    set.wkt_step_index = MessageIndex(idx);
    set.start_time = datetime(start);
    set.timestamp = datetime(start + step.duration.unwrap_or(0));
    if let Some(duration) = step.duration {
        set.set_duration_scaled(duration as f64);
    }
    set.set_type = if step.kind == StepType::Rest {
        SetType::REST
    } else {
        SetType::ACTIVE
    };

    if let (Some(cat), Some(id)) = (step.ex_cat, step.ex_id) {
        set.category = vec![ExerciseCategory(cat)].into();
        set.category_subtype = vec![id].into();
        set.repetitions = step.reps.unwrap_or(0);
        set.set_weight_scaled(step.weight.unwrap_or(0.0) as f64);
        set.weight_display_unit = match weight_unit {
            WeightUnit::Kilograms => FitBaseUnit::KILOGRAM,
            WeightUnit::Pounds => FitBaseUnit::POUND,
        };
    }

    set.into()
}

fn session_end(session: &Session) -> u32 {
    session.date + session.total_elapsed_time
}

fn unit_id(session: &Session) -> Option<u32> {
    let unit_id = Device::unit_id_from_serial(session.device.as_deref()?).ok()?;
    u32::try_from(unit_id).ok()
}

fn first_position(session: &Session) -> Option<(i32, i32)> {
    session
        .additional_data
        .as_ref()?
        .get_coordinates_semicircle()?
        .into_iter()
        .flatten()
        .next()
}

fn sorted_steps(session: &Session) -> Vec<&SessionStep> {
    let mut steps: Vec<_> = session.steps.iter().collect();
    steps.sort_by_key(|s| s.idx);
    steps
}

/// The workout message carries the session name, even for sessions without sets.
fn workout_messages(
    session: &Session,
    lang: Languages,
    weight_unit: WeightUnit,
) -> super::errors::Result<Vec<Message>> {
    if session.steps.is_empty() && session.name.is_empty() {
        return Ok(vec![]);
    }

    let workout = Workout {
        name: session.name.clone(),
        enabled: true,
        notes: session.workout_obj.as_ref().and_then(|w| w.notes.clone()),
        connect_id: session
            .workout_obj
            .as_ref()
            .and_then(|w| w.connect_id.clone()),
        steps: sorted_steps(session)
            .into_iter()
            .enumerate()
            .map(|(idx, step)| workout_step(&session.name, idx as u16, step))
            .collect(),
    };

    let mut messages = workout.to_fit_messages(lang, weight_unit)?;
    let mut fit_workout = FitWorkout::from(&messages[0]);
    fit_workout.sport = Sport(session.sport);
    fit_workout.sub_sport = SubSport(session.sub_sport);
    messages[0] = fit_workout.into();
    messages.insert(0, training_file_message(session, &workout));

    Ok(messages)
}

/// Sets don't keep their start time, so they are laid out back to back from the session start.
fn set_messages(session: &Session, weight_unit: WeightUnit) -> Vec<Message> {
    let mut start = session.date;
    sorted_steps(session)
        .into_iter()
        .enumerate()
        .map(|(idx, step)| {
            let msg = set_message(idx as u16, step, start, weight_unit);
            start += step.duration.unwrap_or(0);
            msg
        })
        .collect()
}

/// The parser drops the first lap, so it is rebuilt here. Laps don't keep their times, so the
/// session is split evenly among them.
fn lap_messages(session: &Session) -> Vec<Message> {
    let mut laps: Vec<_> = session.laps.iter().collect();
    laps.sort_by_key(|l| l.idx);

    let first_position = first_position(session);
    let starts = std::iter::once((first_position.map(|p| p.0), first_position.map(|p| p.1)))
        .chain(laps.iter().map(|l| (l.start_latitude, l.start_longitude)));
    let count = laps.len() + 1;

    starts
        .enumerate()
        .map(|(idx, (lat, long))| {
            let start = time_at(session.date, session.total_elapsed_time, idx, count);
            let end = time_at(session.date, session.total_elapsed_time, idx + 1, count);

            let mut lap = Lap::new();
            lap.message_index = MessageIndex(idx as u16);
            lap.event = typedef::Event::LAP;
            lap.event_type = EventType::STOP;
            lap.lap_trigger = if idx + 1 == count {
                LapTrigger::SESSION_END
            } else {
                LapTrigger::MANUAL
            };
            lap.sport = Sport(session.sport);
            lap.start_time = datetime(start);
            lap.timestamp = datetime(end);
            lap.set_total_elapsed_time_scaled((end - start) as f64);
            lap.set_total_timer_time_scaled((end - start) as f64);
            if let (Some(lat), Some(long)) = (lat, long) {
                lap.start_position_lat = lat;
                lap.start_position_long = long;
            }
            lap.into()
        })
        .collect()
}

fn session_message(session: &Session, num_laps: u16, lang: Languages) -> Message {
    let mut msg = FitSession::new();
    msg.message_index = MessageIndex(0);
    // The session date is its start, which is what devices record in this timestamp
    msg.timestamp = datetime(session.date);
    msg.start_time = datetime(session.date);
    msg.event = typedef::Event::SESSION;
    msg.event_type = EventType::STOP;
    msg.trigger = SessionTrigger::ACTIVITY_END;
    msg.sport = Sport(session.sport);
    msg.sub_sport = SubSport(session.sub_sport);
    msg.sport_profile_name = sport_name(session, lang).into();
    msg.first_lap_index = 0;
    msg.num_laps = num_laps;
    msg.set_total_elapsed_time_scaled(session.total_elapsed_time as f64);
    msg.set_total_timer_time_scaled(session.total_elapsed_time as f64);
    msg.set_active_time_scaled(session.active_time as f64);
    msg.set_training_load_peak_scaled(session.training_load as f64);
    // Only active calories are kept, so the metabolic share is lost
    msg.total_calories = session.active_calories;
    msg.metabolic_calories = 0;

    if let Some((lat, long)) = first_position(session) {
        msg.start_position_lat = lat;
        msg.start_position_long = long;
    }

    if let Some(data) = &session.additional_data {
        if let Some(distance) = data.distance {
            // The scaled setter truncates, which would shift the value on every round trip
            msg.total_distance = (distance * 100.0).round() as u32;
        }

        let hrs: Vec<u32> = data
            .get_heart_rates()
            .unwrap_or_default()
            .into_iter()
            .flatten()
            .filter(|hr| *hr > 0)
            .map(u32::from)
            .collect();
        if let Some(max) = hrs.iter().max() {
            msg.max_heart_rate = *max as u8;
            msg.avg_heart_rate = (hrs.iter().sum::<u32>() / hrs.len() as u32) as u8;
        }
    }

    msg.into()
}

fn activity_message(session: &Session) -> Message {
    let end = session_end(session);
    let offset = Local
        .timestamp_opt(end as i64, 0)
        .single()
        .map_or(0, |d| d.offset().local_minus_utc());

    let mut activity = Activity::new();
    activity.timestamp = datetime(end);
    activity.local_timestamp = LocalDateTime::from_unix_timestamp(end as i64 + offset as i64);
    activity.set_total_timer_time_scaled(session.total_elapsed_time as f64);
    activity.num_sessions = 1;
    activity.r#type = typedef::Activity::MANUAL;
    activity.event = typedef::Event::ACTIVITY;
    activity.event_type = EventType::STOP;
    activity.into()
}

impl ToFitMessages for Session {
    fn file_type(&self) -> typedef::File {
        typedef::File::ACTIVITY
    }

    fn file_key(&self) -> u32 {
        // Sessions coming from a file may not have a device; the date is unique among sessions anyway
        unit_id(self).unwrap_or(self.date)
    }

    /// Inverse of the mapping done in `TryFrom<FitParser> for Session`.
    fn to_fit_messages(
        &self,
        lang: Languages,
        weight_unit: WeightUnit,
    ) -> super::errors::Result<Vec<Message>> {
        let mut messages = Vec::new();

        if let Some(unit_id) = unit_id(self) {
            let mut device = DeviceInfo::new();
            device.timestamp = datetime(self.date);
            device.device_index = DeviceIndex::CREATOR;
            device.serial_number = unit_id;
            messages.push(device.into());
        }

        messages.push(timer_event(self.date, EventType::START));
        messages.push(sport_message(self, lang));
        messages.extend(workout_messages(self, lang, weight_unit)?);

        if let Some(data) = &self.additional_data {
            messages.extend(record_messages(data, self.date, self.total_elapsed_time));

            if let Some(max_hr) = data.max_hr {
                let mut zones = TimeInZone::new();
                zones.timestamp = datetime(session_end(self));
                zones.reference_mesg = MesgNum::SESSION;
                zones.reference_index = MessageIndex(0);
                zones.max_heart_rate = max_hr;
                messages.push(zones.into());
            }
        }

        messages.extend(set_messages(self, weight_unit));

        let laps = lap_messages(self);
        let num_laps = laps.len() as u16;
        messages.extend(laps);

        messages.push(timer_event(session_end(self), EventType::STOP_ALL));
        messages.push(session_message(self, num_laps, lang));
        messages.push(activity_message(self));

        Ok(messages)
    }
}

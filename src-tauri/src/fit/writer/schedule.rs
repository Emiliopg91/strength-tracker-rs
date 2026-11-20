use chrono::{Local, TimeZone};
use rustyfit::{
    profile::{
        mesgdef::{FileId, Schedule},
        typedef::{self, Bool, LocalDateTime},
    },
    proto::Message,
};
use tauri_plugin_log::log::warn;

use crate::{
    dao::{settings::WeightUnit, workout_planning::WorkoutPlanning},
    fit::writer::{FitWriter, ToFitMessages, fnv1a},
    utils::translations::Languages,
};

/// Hour of the day devices and Connect put a scheduled workout at
const SCHEDULED_HOUR: u32 = 12;

/// Noon of the local day of `date` (a UTC timestamp in seconds) as local wall-clock seconds since
/// the Unix epoch, which is how `local_date_time` fields count.
fn local_noon(date: u32) -> Option<i64> {
    let noon = Local
        .timestamp_opt(i64::from(date), 0)
        .single()?
        .date_naive()
        .and_hms_opt(SCHEDULED_HOUR, 0, 0)?;
    Some(noon.and_utc().timestamp())
}

/// `schedule` message of `plan`. Devices find the workout file by the keys of its `file_id`
/// (maker, product, serial and creation time), so they are copied from the one the workout is
/// written with. A workout without a Connect creation time is written with the time of writing,
/// which no schedule can know ahead, so its plans are left out. `workout_obj` must be loaded.
fn schedule_message(plan: &WorkoutPlanning) -> Option<Message> {
    let workout = plan.workout_obj.as_ref().unwrap();
    if workout.file_created().is_none() {
        warn!(
            "Workout '{}' has no Connect creation time, its plan on {} can't be scheduled",
            plan.workout, plan.date
        );
        return None;
    }
    let Some(scheduled) = local_noon(plan.date) else {
        warn!("Invalid date {} of the plan '{}'", plan.date, plan.workout);
        return None;
    };

    let file_id = FileId::from(&FitWriter::from(workout).file_id());

    let mut schedule = Schedule::new();
    schedule.manufacturer = file_id.manufacturer;
    schedule.product = file_id.product;
    schedule.serial_number = file_id.serial_number;
    schedule.time_created = file_id.time_created;
    schedule.completed = if plan.session.is_some() {
        Bool::TRUE
    } else {
        Bool::FALSE
    };
    schedule.r#type = typedef::Schedule::WORKOUT;
    schedule.scheduled_time = LocalDateTime::from_unix_timestamp(scheduled);
    Some(schedule.into())
}

/// Schedule file with every plan of the list, in its order. Picking the plans (the window ahead
/// the device should know) is up to the caller.
impl ToFitMessages for Vec<WorkoutPlanning> {
    fn file_type(&self) -> typedef::File {
        typedef::File::SCHEDULES
    }

    fn file_key(&self) -> u32 {
        // Derived from the content so a changed schedule never looks like the previous file.
        // Never 0, the invalid value of the uint32z serial
        fnv1a(self.iter().flat_map(|p| {
            p.date
                .to_le_bytes()
                .into_iter()
                .chain(p.workout.to_le_bytes())
        }))
        .max(1)
    }

    fn to_fit_messages(
        &self,
        _lang: Languages,
        _weight_unit: WeightUnit,
    ) -> super::errors::Result<Vec<Message>> {
        Ok(self.iter().filter_map(schedule_message).collect())
    }
}

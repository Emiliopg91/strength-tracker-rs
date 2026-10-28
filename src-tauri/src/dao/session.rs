use rusqlite_orm::Entity;
use serde::{Deserialize, Serialize};

use crate::dao::{
    additional_data::{self, AdditionalData},
    device::{self, Device},
    lap::{self, Lap},
    set,
    sub_sport::{self, SubSport},
    workout::{self, Workout},
};

use super::set::Set;

#[derive(Entity, Clone, Serialize, Deserialize)]
#[primary_key(date)]
#[index("name", (name))]
pub struct Session {
    pub date: u32,
    pub name: String,
    pub total_elapsed_time: u32,
    pub active_time: u32,
    pub active_calories: u16,
    pub training_load: u16,
    pub device: Option<String>,
    pub workout: Option<String>,
    pub sport: u8,
    pub sub_sport: u8,
    pub show_exercises_chart: bool,

    #[relationship((date, set::entity::columns::SESSION))]
    pub sets: Vec<Set>,

    #[relationship((date, lap::entity::columns::SESSION))]
    pub laps: Vec<Lap>,

    #[relationship((device, device::entity::columns::SERIAL))]
    pub device_obj: Option<Device>,

    #[relationship((date, additional_data::entity::columns::SESSION))]
    pub additional_data: Option<AdditionalData>,

    #[relationship((sport, sub_sport::entity::columns::SPORT),(sub_sport, sub_sport::entity::columns::ID))]
    pub sub_sport_obj: Option<SubSport>,

    #[relationship((workout, workout::entity::columns::NAME))]
    pub workout_obj: Option<Workout>,
}

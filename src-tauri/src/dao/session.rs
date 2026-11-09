use rusqlite_orm::Entity;
use serde::{Deserialize, Serialize};

use crate::dao::{
    device::{self, Device},
    session_data::{self, SessionData},
    session_lap::{self, SessionLap},
    session_step,
    sub_sport::{self, SubSport},
    workout::{self, Workout},
};

use super::session_step::SessionStep;

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

    #[relationship((date, session_step::entity::columns::SESSION))]
    pub steps: Vec<SessionStep>,

    #[relationship((date, session_lap::entity::columns::SESSION))]
    pub laps: Vec<SessionLap>,

    #[relationship((device, device::entity::columns::SERIAL))]
    pub device_obj: Option<Device>,

    #[relationship((date, session_data::entity::columns::SESSION))]
    pub data: Option<SessionData>,

    #[relationship((sport, sub_sport::entity::columns::SPORT),(sub_sport, sub_sport::entity::columns::ID))]
    pub sub_sport_obj: Option<SubSport>,

    #[relationship((workout, workout::entity::columns::NAME))]
    pub workout_obj: Option<Workout>,
}

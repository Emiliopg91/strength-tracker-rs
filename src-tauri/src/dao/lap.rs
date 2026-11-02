use rusqlite_orm::Entity;
use serde::{Deserialize, Serialize};

#[derive(Entity, Clone, Serialize, Deserialize)]
#[primary_key(session, idx)]
#[index("session", (session))]
pub struct Lap {
    pub session: u32,
    pub idx: u16,
    pub start_latitude: Option<i32>,
    pub start_longitude: Option<i32>,
}

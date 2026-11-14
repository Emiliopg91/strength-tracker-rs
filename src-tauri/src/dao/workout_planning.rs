use rusqlite_orm::Entity;

use crate::dao::workout::{self, Workout};

#[derive(Entity)]
#[entity("workout_planning")]
#[primary_key(date, workout)]
#[index("date", (date))]
#[index("session", (session))]
pub struct WorkoutPlanning {
    pub date: u32,
    pub workout: String,
    /// Session that fulfilled this plan; set to NULL by the DB if that session is deleted
    pub session: Option<u32>,
    pub connect_id: Option<String>,

    #[allow(dead_code)]
    #[relationship((workout, workout::entity::columns::NAME))]
    pub workout_obj: Option<Workout>,
}

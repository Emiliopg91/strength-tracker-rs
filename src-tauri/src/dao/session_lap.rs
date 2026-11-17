use rusqlite_orm::{
    Entity, dao::Repository, rusqlite::Connection, types::value::Value, types::where_clause::Where,
};
use serde::{Deserialize, Serialize};

#[derive(Entity, Clone, Serialize, Deserialize)]
#[entity("session_lap")]
#[primary_key(session, idx)]
#[index("session", (session))]
pub struct SessionLap {
    pub session: u32,
    pub idx: u16,
    pub start_latitude: Option<i32>,
    pub start_longitude: Option<i32>,
}

impl SessionLap {
    /// Laps of every session in `dates`, unordered.
    pub async fn get_by_sessions(
        conn: &Connection,
        dates: &[Value],
    ) -> rusqlite_orm::errors::Result<Vec<SessionLap>> {
        SessionLapRepository::select()
            .where_(Where::In(entity::columns::SESSION, dates.to_vec()))
            .fetch_in(conn)
            .await
    }
}

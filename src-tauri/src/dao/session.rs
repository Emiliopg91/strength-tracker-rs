use std::collections::{BTreeSet, HashMap, HashSet};

use rusqlite_orm::{
    Entity,
    dao::Repository,
    rusqlite::{Connection, Transaction},
    types::{order_by::OrderBy, value::Value, where_clause::Where},
};
use serde::{Deserialize, Serialize};

use crate::{
    dao::{
        device::{self, Device},
        session_data::{self, SessionData, SessionDataRepository},
        session_lap::{self, SessionLap, SessionLapRepository},
        session_step::{self, SessionStepRepository},
        sub_sport::{self, SubSport},
        workout::{self, Workout, WorkoutRepository, entity::columns as workout_cols},
        workout_planning::{WorkoutPlanningRepository, entity::columns as workout_planning_cols},
        workout_step::{WorkoutStep, WorkoutStepRepository},
    },
    utils::time::local_midnight,
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
    pub connect_id: Option<String>,

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

impl Session {
    pub async fn get_dates(conn: &Connection) -> rusqlite_orm::errors::Result<BTreeSet<u32>> {
        Ok(SessionRepository::select()
            .columns(&[entity::columns::DATE])
            .fetch_in(conn)
            .await?
            .iter()
            .map(|row| row.get_u32(entity::columns::DATE.as_ref()))
            .collect())
    }

    /// Connect activity id of the session started at `date`, `None` if it isn't linked to one.
    pub async fn get_connect_id(
        conn: &Connection,
        date: u32,
    ) -> rusqlite_orm::errors::Result<Option<String>> {
        Ok(SessionRepository::select()
            .columns(&[entity::columns::CONNECT_ID])
            .where_(Where::Eq(entity::columns::DATE, date.into()))
            .fetch_one_in(conn)
            .await?
            .and_then(|row| row.get_text_nullable(entity::columns::CONNECT_ID.as_ref())))
    }

    /// Sessions newest first, filtered by the bounds and sport given.
    pub async fn get_sessions(
        conn: &Connection,
        from: Option<i64>,
        to: Option<i64>,
        sport: Option<u8>,
        sub_sport: Option<u8>,
    ) -> rusqlite_orm::errors::Result<Vec<Session>> {
        let mut select_builder =
            SessionRepository::select().order_by(OrderBy::Desc(entity::columns::DATE));

        let mut conditions = Vec::new();
        if let Some(min_date) = from {
            conditions.push(Where::Gte(entity::columns::DATE, min_date.into()))
        }
        if let Some(max_date) = to {
            conditions.push(Where::Lte(entity::columns::DATE, max_date.into()))
        }
        if let Some(sport) = sport {
            conditions.push(Where::Eq(entity::columns::SPORT, sport.into()))
        }
        if let Some(sub_sport) = sub_sport {
            conditions.push(Where::Eq(entity::columns::SUB_SPORT, sub_sport.into()))
        }
        if !conditions.is_empty() {
            select_builder = select_builder.where_(Where::And(conditions))
        }

        select_builder.fetch_in(conn).await
    }

    /// Sessions started at any of `dates`, unordered.
    pub async fn get_by_dates(
        conn: &Connection,
        dates: &[Value],
    ) -> rusqlite_orm::errors::Result<Vec<Session>> {
        SessionRepository::select()
            .where_(Where::In(entity::columns::DATE, dates.to_vec()))
            .fetch_in(conn)
            .await
    }

    /// Which of `dates` already have a session.
    pub async fn get_existing_dates(
        conn: &Connection,
        dates: &[u32],
    ) -> rusqlite_orm::errors::Result<HashSet<u32>> {
        if dates.is_empty() {
            return Ok(HashSet::new());
        }
        Ok(SessionRepository::select()
            .columns(&[entity::columns::DATE])
            .where_(Where::In(
                entity::columns::DATE,
                dates.iter().map(|d| (*d).into()).collect(),
            ))
            .fetch_in(conn)
            .await?
            .iter()
            .map(|row| row.get_u32(entity::columns::DATE.as_ref()))
            .collect())
    }

    /// Names of the sessions started at any of `dates`, by date, within `from_to` if given.
    pub async fn get_names_by_dates(
        conn: &Connection,
        dates: &HashSet<u32>,
        from_to: Option<(u32, u32)>,
    ) -> rusqlite_orm::errors::Result<HashMap<u32, String>> {
        let mut conditions = vec![Where::In(
            entity::columns::DATE,
            dates.iter().map(|d| (*d).into()).collect(),
        )];
        if let Some((from, to)) = from_to {
            conditions.push(Where::Gte(entity::columns::DATE, from.into()));
            conditions.push(Where::Lte(entity::columns::DATE, to.into()));
        }

        Ok(SessionRepository::select()
            .columns(&[entity::columns::DATE, entity::columns::NAME])
            .where_(Where::And(conditions))
            .fetch_in(conn)
            .await?
            .iter()
            .map(|s| {
                (
                    s.get_u32(entity::columns::DATE.as_ref()),
                    s.get_text(entity::columns::NAME.as_ref()),
                )
            })
            .collect())
    }

    /// Sessions recorded following a workout, newest first.
    pub async fn get_workout_sessions(
        conn: &Connection,
    ) -> rusqlite_orm::errors::Result<Vec<Session>> {
        SessionRepository::select()
            .where_(Where::NotNull(entity::columns::WORKOUT))
            .order_by(OrderBy::Desc(entity::columns::DATE))
            .fetch_in(conn)
            .await
    }

    /// Sessions named `name`, newest first.
    pub async fn get_by_name(
        conn: &Connection,
        name: &str,
    ) -> rusqlite_orm::errors::Result<Vec<Session>> {
        SessionRepository::select_by_name_in(
            conn,
            name,
            Some(&[OrderBy::Desc(entity::columns::DATE)]),
        )
        .await
    }

    /// The session with its sets, laps, additional data and workout, `None` if it doesn't exist.
    pub async fn get_full(
        conn: &Connection,
        timestamp: u32,
    ) -> rusqlite_orm::errors::Result<Option<Session>> {
        let Some(mut session) = SessionRepository::select_by_id_in(conn, timestamp).await? else {
            return Ok(None);
        };
        session.fetch_steps_relationship_in(conn).await?;
        session.fetch_laps_relationship_in(conn).await?;
        session.fetch_data_relationship_in(conn).await?;
        if session.workout.is_some() {
            session.fetch_workout_obj_relationship_in(conn).await?;
        }
        Ok(Some(session))
    }

    /// Name of the session, `None` if it doesn't exist.
    pub async fn get_name(
        conn: &Connection,
        timestamp: u32,
    ) -> rusqlite_orm::errors::Result<Option<String>> {
        Ok(SessionRepository::select_by_id_in(conn, timestamp)
            .await?
            .map(|session| session.name))
    }

    /// Links the session to its Garmin Connect activity.
    pub async fn set_connect_id(
        conn: &Connection,
        timestamp: u32,
        id: u64,
    ) -> rusqlite_orm::errors::Result<usize> {
        SessionRepository::update()
            .where_(Where::Eq(entity::columns::DATE, timestamp.into()))
            .set(entity::columns::CONNECT_ID, id.into())
            .execute_in(conn)
            .await
    }

    /// Renames every session of `sessions` to `name`; returns the number of rows changed.
    pub async fn set_name(
        conn: &Connection,
        sessions: &[u32],
        name: &str,
    ) -> rusqlite_orm::errors::Result<usize> {
        if sessions.is_empty() {
            return Ok(0);
        }
        SessionRepository::update()
            .set(entity::columns::NAME, name.into())
            .where_(Where::In(
                entity::columns::DATE,
                sessions.iter().map(|s| (*s).into()).collect(),
            ))
            .execute_in(conn)
            .await
    }

    /// Inserts a session with its sets, laps and additional data, creating its workout (and
    /// steps, if missing) when the session references one, and links it to the plan it fulfils.
    /// Child collections are restored on the session after insertion.
    pub async fn insert_full(
        tx: &Transaction<'_>,
        session: &mut Session,
        handled_workouts: &mut HashSet<String>,
    ) -> rusqlite_orm::errors::Result<()> {
        let mut series = std::mem::take(&mut session.steps);
        let mut laps = std::mem::take(&mut session.laps);
        let mut add_data = session.data.take();

        if let Some(workout) = &session.workout_obj {
            let updated = WorkoutRepository::update()
                .set(workout_cols::NOTES, workout.notes.clone().into())
                .where_(Where::Eq(workout_cols::NAME, workout.name.clone().into()))
                .execute_in(tx)
                .await?;
            if updated == 0 {
                WorkoutRepository::insert()
                    .or_ignore()
                    .item(&mut Workout {
                        name: workout.name.clone(),
                        enabled: true,
                        notes: workout.notes.clone(),
                        steps: Vec::new(),
                        connect_id: None,
                        connect_created_time: None,
                    })
                    .execute_in(tx)
                    .await?;
            }

            if handled_workouts.insert(workout.name.clone())
                && WorkoutStepRepository::count_by_workout_in(tx, &workout.name).await? == 0
            {
                WorkoutStep::insert_all(tx, &mut workout.steps.clone()).await?;
            }
        }

        SessionRepository::insert()
            .item(session)
            .execute_in(tx)
            .await?;

        if !series.is_empty() {
            let mut insert = SessionStepRepository::insert();
            for serie in &mut series {
                insert = insert.item(serie);
            }
            insert.execute_in(tx).await?;
        }

        if let Some(additional_data) = &mut add_data {
            SessionDataRepository::insert()
                .item(additional_data)
                .execute_in(tx)
                .await?;
        }

        if !laps.is_empty() {
            let mut insert = SessionLapRepository::insert();
            for lap in &mut laps {
                insert = insert.item(lap)
            }
            insert.execute_in(tx).await?;
        }

        if let Some(workout) = session.workout.clone()
            && let Some(plan_date) = local_midnight(session.date as i64)
        {
            WorkoutPlanningRepository::update()
                .set(workout_planning_cols::SESSION, session.date.into())
                .where_(Where::And(vec![
                    Where::Eq(workout_planning_cols::DATE, plan_date.into()),
                    Where::Eq(workout_planning_cols::WORKOUT, workout.into()),
                    Where::Null(workout_planning_cols::SESSION),
                ]))
                .execute_in(tx)
                .await?;
        }

        session.steps = series;
        session.laps = laps;
        session.data = add_data;

        Ok(())
    }
}

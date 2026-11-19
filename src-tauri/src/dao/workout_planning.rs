use std::collections::{HashMap, HashSet};

use rusqlite_orm::{
    Entity,
    dao::Repository,
    rusqlite::Connection,
    types::{order_by::OrderBy, where_clause::Where},
};

use crate::dao::workout::{self, Workout, WorkoutRepository};

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

impl WorkoutPlanning {
    /// Plan schedules that still have to be uploaded to Connect.
    pub async fn get_pending_connect(
        conn: &rusqlite_orm::rusqlite::Connection,
    ) -> rusqlite_orm::errors::Result<Vec<(u32, String)>> {
        Ok(WorkoutPlanningRepository::select()
            .where_(Where::And(vec![Where::Null(entity::columns::CONNECT_ID)]))
            .order_by(OrderBy::Asc(entity::columns::DATE))
            .order_by(OrderBy::Asc(entity::columns::WORKOUT))
            .fetch_in(conn)
            .await?
            .into_iter()
            .map(|e| (e.date, e.workout))
            .collect())
    }

    /// Plans dated in `[from, to)`, by date and workout, with their workout loaded.
    pub async fn get_between(
        conn: &rusqlite_orm::rusqlite::Connection,
        from: u32,
        to: u32,
    ) -> rusqlite_orm::errors::Result<Vec<WorkoutPlanning>> {
        let mut plans = WorkoutPlanningRepository::select()
            .where_(Where::And(vec![
                Where::Gte(entity::columns::DATE, from.into()),
                Where::Lt(entity::columns::DATE, to.into()),
            ]))
            .order_by(OrderBy::Asc(entity::columns::DATE))
            .order_by(OrderBy::Asc(entity::columns::WORKOUT))
            .fetch_in(conn)
            .await?;
        if plans.is_empty() {
            return Ok(plans);
        }

        // Their workouts in a single query, instead of one per plan as the relationship fetcher would
        let names: HashSet<&str> = plans.iter().map(|plan| plan.workout.as_str()).collect();
        let workouts: HashMap<String, Workout> = WorkoutRepository::select()
            .where_(Where::In(
                workout::entity::columns::NAME,
                names.into_iter().map(Into::into).collect(),
            ))
            .fetch_in(conn)
            .await?
            .into_iter()
            .map(|workout| (workout.name.clone(), workout))
            .collect();
        for plan in &mut plans {
            plan.workout_obj = workouts.get(&plan.workout).cloned();
        }
        Ok(plans)
    }

    /// Every plan.
    pub async fn get_all(conn: &Connection) -> rusqlite_orm::errors::Result<Vec<WorkoutPlanning>> {
        WorkoutPlanningRepository::select().fetch_in(conn).await
    }

    /// The plan of `workout` on `date`, `None` if there is none.
    pub async fn get(
        conn: &Connection,
        date: u32,
        workout: &str,
    ) -> rusqlite_orm::errors::Result<Option<WorkoutPlanning>> {
        WorkoutPlanningRepository::select_by_id_in(conn, date, workout).await
    }

    /// The earliest plan not done yet from `from` on.
    pub async fn get_next(
        conn: &Connection,
        from: Option<u32>,
    ) -> rusqlite_orm::errors::Result<Option<WorkoutPlanning>> {
        WorkoutPlanningRepository::select()
            .where_(Where::And(vec![
                Where::Gte(entity::columns::DATE, from.into()),
                Where::Null(entity::columns::SESSION),
            ]))
            .order_by(OrderBy::Asc(entity::columns::DATE))
            .fetch_one_in(conn)
            .await
    }

    /// Plans `workout` on `date`.
    pub async fn insert(
        conn: &Connection,
        date: u32,
        workout: &str,
    ) -> rusqlite_orm::errors::Result<()> {
        let mut item = WorkoutPlanning {
            date,
            workout: workout.to_string(),
            workout_obj: None,
            session: None,
            connect_id: None,
        };

        WorkoutPlanningRepository::insert()
            .item(&mut item)
            .execute_in(conn)
            .await
            .map(|_| ())
    }

    /// Stores the Connect schedule id of the plan of `workout` on `date`.
    pub async fn set_schedule_id(
        conn: &Connection,
        date: u32,
        workout: &str,
        schedule_id: &str,
    ) -> rusqlite_orm::errors::Result<()> {
        WorkoutPlanningRepository::update()
            .set(entity::columns::CONNECT_ID, schedule_id.to_string().into())
            .where_(Where::And(vec![
                Where::Eq(entity::columns::DATE, date.into()),
                Where::Eq(entity::columns::WORKOUT, workout.into()),
            ]))
            .execute_in(conn)
            .await
            .map(|_| ())
    }
}

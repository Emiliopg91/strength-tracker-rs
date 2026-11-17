use std::collections::BTreeSet;

use rusqlite_orm::{Entity, dao::Repository, rusqlite::Connection, types::where_clause::Where};
use serde::{Deserialize, Serialize};

#[derive(Clone, Entity, Serialize, Deserialize)]
#[entity("exercise_category")]
#[primary_key(id)]
pub struct ExerciseCategory {
    pub id: u16,
    pub name: String,
}

impl ExerciseCategory {
    /// The categories with the given ids.
    pub async fn get_by_ids(
        conn: &Connection,
        category_ids: &BTreeSet<u16>,
    ) -> rusqlite_orm::errors::Result<Vec<ExerciseCategory>> {
        if category_ids.is_empty() {
            return Ok(Vec::new());
        }
        ExerciseCategoryRepository::select()
            .where_(Where::In(
                entity::columns::ID,
                category_ids.iter().map(|id| (*id).into()).collect(),
            ))
            .fetch_in(conn)
            .await
    }

    /// Every exercise category.
    pub async fn get_all(conn: &Connection) -> rusqlite_orm::errors::Result<Vec<ExerciseCategory>> {
        ExerciseCategoryRepository::select().fetch_in(conn).await
    }
}

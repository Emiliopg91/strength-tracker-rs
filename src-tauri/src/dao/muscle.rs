use rusqlite_orm::Entity;
use serde::{Deserialize, Serialize};

#[derive(Entity, Clone, Deserialize, Serialize)]
pub struct Muscle {
    name: String,
}

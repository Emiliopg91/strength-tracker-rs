use rusqlite_orm::{
    dao::Repository,
    database::{DatabasePool, builder::DatabaseConnectionBuilder},
    rusqlite::Transaction,
};

use crate::{
    dao::{
        additional_data::{AdditionalData, AdditionalDataRepository},
        lap::{Lap, LapRepository},
        session::{Session, SessionRepository},
        set::{Set, SetRepository},
    },
    schema_ddls,
};

/// Strength training sport/sub-sport pair, present in the base schema seed data.
pub const STRENGTH_SPORT: u8 = 10;
pub const STRENGTH_SUB_SPORT: u8 = 20;

/// Builds an in-memory database with the full schema applied, mirroring `initialize()` in `lib.rs`.
pub fn test_db() -> DatabasePool {
    let database = DatabaseConnectionBuilder::default()
        .enable_foreign_keys()
        .build("test")
        .expect("in-memory database");

    database
        .create_schema(&schema_ddls())
        .expect("schema creation");

    database
}

pub fn session(date: u32) -> Session {
    Session {
        date,
        name: format!("Session {date}"),
        total_elapsed_time: 3600,
        active_time: 3000,
        active_calories: 100,
        training_load: 80,
        device: None,
        workout: None,
        sport: STRENGTH_SPORT,
        sub_sport: STRENGTH_SUB_SPORT,
        sets: Vec::new(),
        laps: Vec::new(),
        device_obj: None,
        additional_data: None,
        sub_sport_obj: None,
        workout_obj: None,
        show_exercises_chart: true,
    }
}

pub fn set(session: u32, idx: u8, exercise: (u16, u16), reps: u16, weight: f32) -> Set {
    Set {
        session,
        idx,
        ex_cat: exercise.0,
        ex_id: exercise.1,
        reps,
        weight,
        pr: false,
        exercise: None,
        duration: 0,
    }
}

pub fn additional_data(session: u32) -> AdditionalData {
    AdditionalData {
        session,
        heart_rates: None,
        coordinates: None,
        speeds: None,
        altitudes: None,
        distance: None,
        notes: None,
        max_hr: None,
    }
}

pub fn insert_session(tx: &Transaction, mut session: Session) {
    let mut sets = std::mem::take(&mut session.sets);
    let mut laps = std::mem::take(&mut session.laps);
    let additional_data = session.additional_data.take();

    SessionRepository::insert()
        .item(&mut session)
        .execute_in(tx)
        .expect("session insert");

    if !sets.is_empty() {
        let mut insert = SetRepository::insert();
        for set in &mut sets {
            insert = insert.item(set);
        }
        insert.execute_in(tx).expect("sets insert");
    }

    if !laps.is_empty() {
        let mut insert = LapRepository::insert();
        for lap in &mut laps {
            insert = insert.item(lap);
        }
        insert.execute_in(tx).expect("laps insert");
    }

    if let Some(mut additional_data) = additional_data {
        AdditionalDataRepository::insert()
            .item(&mut additional_data)
            .execute_in(tx)
            .expect("additional data insert");
    }
}

pub fn lap(session: u32, idx: u16, position: Option<(i32, i32)>) -> Lap {
    Lap {
        session,
        idx,
        start_latitude: position.map(|p| p.0),
        start_longitude: position.map(|p| p.1),
    }
}

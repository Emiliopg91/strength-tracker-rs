use std::{
    borrow::Borrow,
    collections::{HashMap, HashSet},
};

use rusqlite_orm::{
    Entity,
    dao::Repository,
    rusqlite::{Connection, Transaction},
    types::{order_by::OrderBy, value::Value, where_clause::Where},
};
use serde::{Deserialize, Serialize};

use crate::dao::{
    exercise::{self},
    session::{SessionRepository, entity::columns as session_cols},
    workout_step::StepType,
};

use super::exercise::Exercise;

#[derive(Entity, Clone, Serialize, Deserialize)]
#[entity("session_step")]
#[primary_key(session, idx)]
#[index("session", (session))]
#[index("personal_records", (pr))]
#[index("session_pr", (session, pr))]
#[index("exercise", (ex_cat, ex_id))]
#[unique("exercise_personal_record", (ex_cat, ex_id), (pr=true))]
pub struct SessionStep {
    pub session: u32,
    pub idx: u8,
    #[column("exercise_category")]
    pub ex_cat: Option<u16>,
    #[column("exercise_id")]
    pub ex_id: Option<u16>,
    pub reps: Option<u16>,
    pub reps_ref: Option<u16>,
    pub weight: Option<f32>,
    pub weight_ref: Option<f32>,
    pub pr: bool,
    pub duration: Option<u32>,
    pub duration_ref: Option<u32>,
    pub kind: StepType,

    #[relationship((ex_cat, exercise::entity::columns::CATEGORY),(ex_id, exercise::entity::columns::ID))]
    #[allow(dead_code)]
    pub exercise: Option<Exercise>,
}

/// Values of `(category, id)` keys, for a `Where::InMultiple` over the exercise columns.
/// A set competing for an exercise's personal record.
struct PrCandidate {
    weight: Option<f32>,
    reps: Option<u16>,
    session: u32,
    idx: u8,
}

impl PrCandidate {
    /// Whether it beats `other` as a record: heavier, then more reps, then earlier (session, then
    /// index). A missing weight or reps loses to any value, as in SQL's ordering.
    fn beats(&self, other: &Self) -> bool {
        self.weight
            .partial_cmp(&other.weight)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(self.reps.cmp(&other.reps))
            .then((other.session, other.idx).cmp(&(self.session, self.idx)))
            .is_gt()
    }
}

pub(crate) fn exercise_key_values(
    keys: impl IntoIterator<Item = impl Borrow<(u16, u16)>>,
) -> Vec<Vec<Value>> {
    keys.into_iter()
        .map(|key| {
            let (cat, id) = *key.borrow();
            vec![cat.into(), id.into()]
        })
        .collect()
}

impl SessionStep {
    /// Sets of the exercise, newest session first and in order within each session.
    pub async fn get_by_exercise(
        conn: &Connection,
        category: u16,
        id: u16,
    ) -> rusqlite_orm::errors::Result<Vec<SessionStep>> {
        SessionStepRepository::select_by_exercise_in(
            conn,
            Some(category),
            Some(id),
            Some(&[
                OrderBy::Desc(entity::columns::SESSION),
                OrderBy::Asc(entity::columns::IDX),
            ]),
        )
        .await
    }

    /// Steps of each exercise in `keys`, newest session first and by index within a session, in a
    /// single query. Exercises without steps are left out.
    pub async fn get_by_exercises<'a>(
        conn: &Connection,
        keys: impl IntoIterator<Item = &'a (u16, u16)>,
    ) -> rusqlite_orm::errors::Result<HashMap<(u16, u16), Vec<SessionStep>>> {
        let keys = exercise_key_values(keys);
        if keys.is_empty() {
            return Ok(HashMap::new());
        }
        let mut by_exercise: HashMap<(u16, u16), Vec<SessionStep>> = HashMap::new();
        for step in SessionStepRepository::select()
            .where_(Where::InMultiple(
                vec![entity::columns::EX_CAT, entity::columns::EX_ID],
                keys,
            ))
            .order_by(OrderBy::Desc(entity::columns::SESSION))
            .order_by(OrderBy::Asc(entity::columns::IDX))
            .fetch_in(conn)
            .await?
        {
            if let Some(key) = step.ex_cat.zip(step.ex_id) {
                by_exercise.entry(key).or_default().push(step);
            }
        }
        Ok(by_exercise)
    }

    /// Steps of every session in `dates`, unordered.
    pub async fn get_by_sessions(
        conn: &Connection,
        dates: &[Value],
    ) -> rusqlite_orm::errors::Result<Vec<SessionStep>> {
        SessionStepRepository::select()
            .where_(Where::In(entity::columns::SESSION, dates.to_vec()))
            .fetch_in(conn)
            .await
    }

    /// The personal record set of every exercise that has one.
    pub async fn get_personal_records(
        conn: &Connection,
    ) -> rusqlite_orm::errors::Result<Vec<SessionStep>> {
        // Only exercise sets can hold a record; filtering by exercise lets SQLite scan the
        // partial SESSION_STEP_EXERCISE_PR index, which holds just the records
        SessionStepRepository::select()
            .where_(Where::And(vec![
                Where::Eq(entity::columns::PR, true.into()),
                Where::NotNull(entity::columns::EX_CAT),
                Where::NotNull(entity::columns::EX_ID),
            ]))
            .fetch_in(conn)
            .await
    }

    /// Keys of every exercise with some recorded set.
    #[cfg(debug_assertions)]
    pub async fn get_performed_exercises(
        conn: &Connection,
    ) -> rusqlite_orm::errors::Result<HashSet<(u16, u16)>> {
        Ok(SessionStepRepository::select()
            .distinct(&[entity::columns::EX_CAT, entity::columns::EX_ID])
            .fetch_in(conn)
            .await?
            .iter()
            .filter_map(|r| {
                r.get_u16_nullable(entity::columns::EX_CAT.as_ref())
                    .zip(r.get_u16_nullable(entity::columns::EX_ID.as_ref()))
            })
            .collect())
    }

    /// Exercises of the steps `idxs` of the session; rest steps, without exercise, are skipped.
    pub async fn get_step_exercises(
        conn: &Connection,
        session: u32,
        idxs: &[u8],
    ) -> rusqlite_orm::errors::Result<HashSet<(u16, u16)>> {
        Ok(SessionStepRepository::select()
            .columns(&[entity::columns::EX_CAT, entity::columns::EX_ID])
            .where_(Where::And(vec![
                Where::Eq(entity::columns::SESSION, session.into()),
                Where::In(
                    entity::columns::IDX,
                    idxs.iter().map(|idx| (*idx).into()).collect(),
                ),
            ]))
            .fetch_in(conn)
            .await?
            .iter()
            .filter_map(|row| {
                row.get_u16_nullable(entity::columns::EX_CAT.as_ref())
                    .zip(row.get_u16_nullable(entity::columns::EX_ID.as_ref()))
            })
            .collect())
    }

    /// Overwrites the exercise, reps and weight of the step `idx` of the session.
    pub async fn update_set(
        tx: &Transaction<'_>,
        session: u32,
        idx: u8,
        exercise: (Option<u16>, Option<u16>),
        reps: Option<u16>,
        weight: Option<f32>,
    ) -> rusqlite_orm::errors::Result<usize> {
        SessionStepRepository::update()
            .set(entity::columns::REPS, reps.into())
            .set(entity::columns::WEIGHT, weight.into())
            .set(entity::columns::EX_CAT, exercise.0.into())
            .set(entity::columns::EX_ID, exercise.1.into())
            .where_(Where::And(vec![
                Where::Eq(entity::columns::SESSION, session.into()),
                Where::Eq(entity::columns::IDX, idx.into()),
            ]))
            .execute_in(tx)
            .await
    }

    /// Maps every session that has sets (within `[min_date, max_date]`, each bound optional) to
    /// whether any of them is a personal record.
    pub async fn get_set_flags(
        conn: &Connection,
        min_date: Option<i32>,
        max_date: Option<i32>,
    ) -> rusqlite_orm::errors::Result<HashMap<u32, bool>> {
        let mut select_builder = SessionStepRepository::select()
            .distinct(&[entity::columns::SESSION, entity::columns::PR]);

        // `where_` replaces the previous condition, so bounds must be combined in a single `And`
        let mut conditions = Vec::new();
        if let Some(min_date) = min_date {
            conditions.push(Where::Gte(entity::columns::SESSION, min_date.into()));
        }
        if let Some(max_date) = max_date {
            conditions.push(Where::Lte(entity::columns::SESSION, max_date.into()));
        }
        if !conditions.is_empty() {
            select_builder = select_builder.where_(Where::And(conditions));
        }

        let mut flags: HashMap<u32, bool> = HashMap::new();
        for row in select_builder.fetch_in(conn).await? {
            let session = row.get_u32(entity::columns::SESSION.as_ref());
            let pr = row.get_bool(entity::columns::PR.as_ref());
            *flags.entry(session).or_default() |= pr;
        }
        Ok(flags)
    }

    /// Number of personal records of every session from `since` on that holds any.
    pub async fn get_pr_counts_since(
        conn: &Connection,
        since: i64,
    ) -> rusqlite_orm::errors::Result<HashMap<u32, u16>> {
        let mut pr_counts: HashMap<u32, u16> = HashMap::new();
        for pr in SessionStepRepository::select()
            .columns(&[entity::columns::SESSION])
            .where_(Where::And(vec![
                Where::Eq(entity::columns::PR, true.into()),
                Where::Gte(entity::columns::SESSION, since.into()),
            ]))
            .fetch_in(conn)
            .await?
        {
            *pr_counts
                .entry(pr.get_u32(entity::columns::SESSION.as_ref()))
                .or_default() += 1;
        }
        Ok(pr_counts)
    }

    /// Exercise sets of the latest session of each exercise in `keys`, as
    /// `(exercise, session, weight, reps)`, a missing weight or reps counting as 0.
    pub async fn get_latest_exercise_sets(
        conn: &Connection,
        keys: &HashSet<(u16, u16)>,
    ) -> rusqlite_orm::errors::Result<Vec<((u16, u16), u32, f32, u16)>> {
        if keys.is_empty() {
            return Ok(Vec::new());
        }

        // Latest session of each exercise
        let latest_sessions = SessionStepRepository::select()
            .columns(&[
                entity::columns::EX_CAT.into(),
                entity::columns::EX_ID.into(),
                entity::columns::SESSION.max(),
            ])
            .where_(Where::And(vec![
                Where::Eq(entity::columns::KIND, StepType::Exercise.into()),
                Where::InMultiple(
                    vec![entity::columns::EX_CAT, entity::columns::EX_ID],
                    exercise_key_values(keys),
                ),
            ]))
            .group_by(&[entity::columns::EX_CAT, entity::columns::EX_ID])
            .to_subquery();

        Ok(SessionStepRepository::select()
            .columns(&[
                entity::columns::EX_CAT,
                entity::columns::EX_ID,
                entity::columns::SESSION,
                entity::columns::WEIGHT,
                entity::columns::REPS,
            ])
            .where_(Where::And(vec![
                Where::Eq(entity::columns::KIND, StepType::Exercise.into()),
                Where::InMultipleSub(
                    vec![
                        entity::columns::EX_CAT,
                        entity::columns::EX_ID,
                        entity::columns::SESSION,
                    ],
                    latest_sessions,
                ),
            ]))
            .fetch_in(conn)
            .await?
            .iter()
            .map(|row| {
                (
                    (
                        row.get_u16(entity::columns::EX_CAT.as_ref()),
                        row.get_u16(entity::columns::EX_ID.as_ref()),
                    ),
                    row.get_u32(entity::columns::SESSION.as_ref()),
                    row.get_f32_nullable(entity::columns::WEIGHT.as_ref())
                        .unwrap_or_default(),
                    row.get_u16_nullable(entity::columns::REPS.as_ref())
                        .unwrap_or_default(),
                )
            })
            .collect())
    }

    /// Volume (reps by weight) lifted in every session of the workout `name` that has sets.
    pub async fn get_volume_by_session(
        conn: &Connection,
        name: &str,
    ) -> rusqlite_orm::errors::Result<HashMap<u32, f32>> {
        let series = SessionStepRepository::select()
            .columns(&[
                entity::columns::SESSION,
                entity::columns::REPS,
                entity::columns::WEIGHT,
            ])
            .where_(Where::And(vec![
                // Rest steps have neither reps nor weight
                Where::NotEq(entity::columns::KIND, StepType::Rest.into()),
                Where::InSub(
                    entity::columns::SESSION,
                    SessionRepository::select()
                        .columns(&[session_cols::DATE])
                        .where_(Where::Eq(session_cols::NAME, name.into()))
                        .to_subquery(),
                ),
            ]))
            .fetch_in(conn)
            .await?;

        let mut volume_by_session = HashMap::new();
        for serie in &series {
            *volume_by_session
                .entry(serie.get_u32(entity::columns::SESSION.as_ref()))
                .or_insert(0.0) += serie
                .get_u16_nullable(entity::columns::REPS.as_ref())
                .unwrap_or_default() as f32
                * serie
                    .get_f32_nullable(entity::columns::WEIGHT.as_ref())
                    .unwrap_or_default();
        }
        Ok(volume_by_session)
    }

    /// Recomputes the `pr` flag for each affected exercise. Returns whether any of `sessions` holds
    /// a new record.
    pub async fn update_prs(
        tx: &Transaction<'_>,
        exercises: &HashSet<(u16, u16)>,
        sessions: &[u32],
    ) -> rusqlite_orm::errors::Result<bool> {
        if exercises.is_empty() {
            return Ok(false);
        }

        // A single query for every set of the exercises; the best of each is picked here
        // (heaviest, then most reps, then earliest), as one query per exercise would
        let mut best: HashMap<(u16, u16), PrCandidate> = HashMap::new();
        for row in SessionStepRepository::select()
            .columns(&[
                entity::columns::EX_CAT,
                entity::columns::EX_ID,
                entity::columns::WEIGHT,
                entity::columns::REPS,
                entity::columns::SESSION,
                entity::columns::IDX,
            ])
            .where_(Where::InMultiple(
                vec![entity::columns::EX_CAT, entity::columns::EX_ID],
                exercise_key_values(exercises),
            ))
            .fetch_in(tx)
            .await?
        {
            let key = (
                row.get_u16(entity::columns::EX_CAT.as_ref()),
                row.get_u16(entity::columns::EX_ID.as_ref()),
            );
            let set = PrCandidate {
                weight: row.get_f32_nullable(entity::columns::WEIGHT.as_ref()),
                reps: row.get_u16_nullable(entity::columns::REPS.as_ref()),
                session: row.get_u32(entity::columns::SESSION.as_ref()),
                idx: row.get_u8(entity::columns::IDX.as_ref()),
            };
            if best.get(&key).is_none_or(|current| set.beats(current)) {
                best.insert(key, set);
            }
        }

        let new_prs = best.values().any(|pr| sessions.contains(&pr.session));
        let update_false_conditions = exercise_key_values(exercises);
        let update_true_conditions: Vec<Vec<Value>> = best
            .into_values()
            .map(|pr| vec![pr.session.into(), pr.idx.into()])
            .collect();

        if !update_true_conditions.is_empty() {
            SessionStepRepository::update()
                .set(entity::columns::PR, false.into())
                .where_(Where::And(vec![
                    Where::InMultiple(
                        vec![entity::columns::EX_CAT, entity::columns::EX_ID],
                        update_false_conditions,
                    ),
                    Where::Eq(entity::columns::PR, true.into()),
                ]))
                .execute_in(tx)
                .await?;
            SessionStepRepository::update()
                .set(entity::columns::PR, true.into())
                .where_(Where::InMultiple(
                    vec![entity::columns::SESSION, entity::columns::IDX],
                    update_true_conditions,
                ))
                .execute_in(tx)
                .await?;
        }

        Ok(new_prs)
    }
}

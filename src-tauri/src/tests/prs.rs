use std::collections::HashSet;

use rusqlite_orm::{database::DatabasePool, types::order_by::OrderBy};

use crate::{
    dao::session_step::{self, SessionStep, SessionStepRepository},
    logic::exercises::queries::update_prs,
    tests::common,
};

const SQUAT: (u16, u16) = (28, 0);
const BENCH: (u16, u16) = (0, 0);

async fn pr_sets(db: &DatabasePool) -> Vec<(u32, u8, u16, u16)> {
    db.run_in_connection(move |conn| {
        Ok(SessionStepRepository::select_by_personal_records_in(
            conn,
            true,
            Some(&[OrderBy::Asc(session_step::entity::columns::EX_CAT)]),
        )?
        .into_iter()
        .map(|s| (s.session, s.idx, s.ex_cat.unwrap(), s.ex_id.unwrap()))
        .collect())
    })
    .await
    .unwrap()
}

async fn insert_and_update(db: &DatabasePool, sessions: Vec<(u32, Vec<SessionStep>)>) {
    db.run_in_transaction(move |tx| {
        let mut exercises = HashSet::new();
        for (date, sets) in &sessions {
            let mut session = common::session(*date);
            for s in sets {
                exercises.insert((s.ex_cat.unwrap(), s.ex_id.unwrap()));
            }
            session.steps = sets.clone();
            common::insert_session(tx, session);
        }
        update_prs(tx, &exercises, &[])?;
        Ok(())
    })
    .await
    .unwrap();
}

#[tokio::test]
async fn heaviest_set_wins_over_more_reps() {
    let db = common::test_db().await;
    insert_and_update(
        &db,
        vec![
            (1, vec![common::set(1, 0, SQUAT, 10, 100.0)]),
            (2, vec![common::set(2, 0, SQUAT, 1, 120.0)]),
        ],
    )
    .await;

    assert_eq!(pr_sets(&db).await, vec![(2, 0, SQUAT.0, SQUAT.1)]);
}

#[tokio::test]
async fn same_weight_more_reps_wins() {
    let db = common::test_db().await;
    insert_and_update(
        &db,
        vec![(
            1,
            vec![
                common::set(1, 0, SQUAT, 5, 100.0),
                common::set(1, 1, SQUAT, 8, 100.0),
                common::set(1, 2, SQUAT, 6, 100.0),
            ],
        )],
    )
    .await;

    assert_eq!(pr_sets(&db).await, vec![(1, 1, SQUAT.0, SQUAT.1)]);
}

#[tokio::test]
async fn tie_goes_to_the_earliest_set() {
    let db = common::test_db().await;
    insert_and_update(
        &db,
        vec![
            (2, vec![common::set(2, 0, SQUAT, 5, 100.0)]),
            (
                1,
                vec![
                    common::set(1, 0, SQUAT, 3, 80.0),
                    common::set(1, 1, SQUAT, 5, 100.0),
                    common::set(1, 2, SQUAT, 5, 100.0),
                ],
            ),
        ],
    )
    .await;

    assert_eq!(pr_sets(&db).await, vec![(1, 1, SQUAT.0, SQUAT.1)]);
}

#[tokio::test]
async fn one_pr_per_exercise() {
    let db = common::test_db().await;
    insert_and_update(
        &db,
        vec![(
            1,
            vec![
                common::set(1, 0, BENCH, 5, 80.0),
                common::set(1, 1, BENCH, 5, 85.0),
                common::set(1, 2, SQUAT, 5, 120.0),
                common::set(1, 3, SQUAT, 5, 110.0),
            ],
        )],
    )
    .await;

    assert_eq!(
        pr_sets(&db).await,
        vec![(1, 1, BENCH.0, BENCH.1), (1, 2, SQUAT.0, SQUAT.1)]
    );
}

#[tokio::test]
async fn new_record_moves_the_flag() {
    let db = common::test_db().await;
    insert_and_update(&db, vec![(1, vec![common::set(1, 0, SQUAT, 5, 100.0)])]).await;
    assert_eq!(pr_sets(&db).await, vec![(1, 0, SQUAT.0, SQUAT.1)]);

    insert_and_update(&db, vec![(2, vec![common::set(2, 0, SQUAT, 5, 105.0)])]).await;
    assert_eq!(pr_sets(&db).await, vec![(2, 0, SQUAT.0, SQUAT.1)]);
}

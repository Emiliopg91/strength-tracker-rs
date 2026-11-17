pub mod app;
pub mod body_metrics;
pub mod connect;
pub mod devices;
pub mod exercises;
pub mod export;
pub mod notifications;
pub mod prompts;
pub mod report;
pub mod sessions;
pub mod workouts;

use std::collections::HashSet;

use crate::dao::exercise::Exercise;

/// Muscles worked by `exercises`, as primary and secondary ones; a muscle primary for any of them
/// is only listed as primary.
pub(crate) fn split_muscles(exercises: &[Exercise]) -> (Vec<String>, Vec<String>) {
    let mut primary = HashSet::new();
    let mut secondary = HashSet::new();
    for muscle in exercises.iter().flat_map(|e| &e.muscles) {
        if muscle.is_primary {
            primary.insert(muscle.muscle.clone());
        } else {
            secondary.insert(muscle.muscle.clone());
        }
    }

    let secondary = secondary
        .into_iter()
        .filter(|m| !primary.contains(m))
        .collect();
    (primary.into_iter().collect(), secondary)
}

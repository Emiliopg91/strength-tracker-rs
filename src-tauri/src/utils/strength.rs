/// Sets above this many reps estimate the 1RM as if they had this many.
const MAX_ESTIMATION_REPS: u16 = 20;

/// Load step used to round when the exercise has no weight increment.
const DEFAULT_INCREMENT: f32 = 0.5;

/// Estimated 1RM of a set, truncated to whole kg: Brzycki up to 10 reps, a linear fit beyond.
/// Unilateral sets count both sides, so half their reps. Mirrors `SessionUtils.estimate1rm`.
pub fn estimate_1rm(weight: f32, reps: u16, unilateral: bool) -> f32 {
    let reps = if unilateral { reps / 2 } else { reps };
    if reps <= 1 {
        return weight.trunc();
    }

    let r = reps.min(MAX_ESTIMATION_REPS) as f64;
    let weight = weight as f64;
    let estimate = if r <= 10.0 {
        weight * 36.0 / (37.0 - r)
    } else {
        100.0 * weight / (101.3 - 2.67123 * r)
    };
    estimate.trunc() as f32
}

/// `pct` percent of `one_rm`, rounded to the nearest multiple of `increment` (0.5 kg without one).
pub fn percent_load(one_rm: f32, pct: u8, increment: f32) -> f32 {
    let increment = if increment > 0.0 {
        increment
    } else {
        DEFAULT_INCREMENT
    };
    let load = one_rm * pct as f32 / 100.0;
    // Rounded to tenths too: multiples of increments like 1.25 pick up float noise
    ((load / increment).round() * increment * 10.0).round() / 10.0
}

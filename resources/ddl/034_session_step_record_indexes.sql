--- Record lookups: index the exercise sets in record order and drop the index on the record flag alone

CREATE INDEX SESSION_STEP_EXERCISE_RECORD ON SESSION_STEP(exercise_category, exercise_id, weight DESC, reps DESC, session, idx);
DROP INDEX SESSION_STEP_EXERCISE;
DROP INDEX SESSION_STEP_PR;

-- The record of an exercise is read straight from SESSION_STEP_EXERCISE_RECORD instead of sorting
-- its sets, which also covers the lookups by exercise of SESSION_STEP_EXERCISE; records are always
-- looked up by exercise too, which scans the partial SESSION_STEP_EXERCISE_PR instead of SESSION_STEP_PR

--- Index tuning: drop unused or redundant indexes and add the missing ones

-- Duplicates the leading column of the primary key (workout, idx)
DROP INDEX WORKOUT_STEP_WORKOUT;

-- The ORM filters with `pr=true`, which SQLite doesn't take as implying `pr = 1`, so the partial
-- index only enforced uniqueness. Lookups by exercise now use it; those by record flag alone keep
-- using SESSION_STEP_PR
DROP INDEX SESSION_STEP_EXERCISE_PR;
CREATE UNIQUE INDEX SESSION_STEP_EXERCISE_PR ON SESSION_STEP(exercise_category, exercise_id) WHERE pr = true;

-- Lookups by exercise and the cascade from EXERCISE skip-scanned the (muscle, ...) primary key
CREATE INDEX MUSCLE_EXERCISE_EXERCISE ON MUSCLE_EXERCISE(exercise_category, exercise_id);

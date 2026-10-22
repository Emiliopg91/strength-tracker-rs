--- Per hand column for exercise table

ALTER TABLE exercise ADD COLUMN unilateral BOOLEAN NOT NULL DEFAULT FALSE;
ALTER TABLE exercise ADD COLUMN progression BOOLEAN NOT NULL DEFAULT TRUE;
ALTER TABLE exercise ADD COLUMN min_range INTEGER NOT NULL DEFAULT 8;
ALTER TABLE exercise ADD COLUMN max_range INTEGER NOT NULL DEFAULT 12;

ALTER TABLE workout RENAME enabled TO suggest;

ALTER TABLE workout_device ADD COLUMN pending BOOLEAN NOT NULL DEFAULT TRUE;
INSERT OR REPLACE INTO workout_device (workout, device, pending)
    SELECT w.name, d.serial, TRUE FROM workout w CROSS JOIN device d;
    

CREATE INDEX WORKOUT_STEP_EXERCISE ON WORKOUT_STEP(exercise_category, exercise_id);
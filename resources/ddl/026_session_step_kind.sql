--- Replace session_step rest flag with the step kind, as in workout_step

ALTER TABLE SESSION_STEP ADD COLUMN kind TEXT NOT NULL DEFAULT 'Exercise';

UPDATE SESSION_STEP SET kind = CASE WHEN rest THEN 'Rest' ELSE 'Exercise' END;

ALTER TABLE SESSION_STEP DROP COLUMN rest;

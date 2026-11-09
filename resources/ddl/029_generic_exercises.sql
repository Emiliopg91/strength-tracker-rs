--- Generic exercises: a FIT step with a category but no exercise name (65535) is the generic exercise
--- of that category (e.g. "Hyperextension"), which Connect stores with an empty exerciseName

-- One per category, so any generic coming from a device can be stored. The empty name is what
-- Connect expects for them
INSERT INTO EXERCISE (category, id, name)
SELECT id, 65535, '' FROM EXERCISE_CATEGORY;

-- Muscles of the generics Connect lists in its catalog
INSERT INTO MUSCLE_EXERCISE (muscle, exercise_category, exercise_id, is_primary) VALUES
    ('chest', 0, 65535, 1), ('shoulders', 0, 65535, 0), ('triceps', 0, 65535, 0), ('calves', 1, 65535, 1),
    ('forearm', 3, 65535, 1), ('obliques', 4, 65535, 1), ('abs', 4, 65535, 0), ('abs', 5, 65535, 1),
    ('obliques', 5, 65535, 1), ('abductors', 5, 65535, 0), ('biceps', 7, 65535, 1), ('forearm', 7, 65535, 0),
    ('hamstrings', 8, 65535, 1), ('lower_back', 8, 65535, 1), ('lats', 8, 65535, 0), ('traps', 8, 65535, 0),
    ('forearm', 8, 65535, 0), ('quads', 8, 65535, 0), ('glutes', 8, 65535, 0), ('adductors', 8, 65535, 0),
    ('hips', 11, 65535, 1), ('abductors', 11, 65535, 1), ('adductors', 11, 65535, 1), ('quads', 12, 65535, 1),
    ('shoulders', 12, 65535, 1), ('glutes', 12, 65535, 0), ('hamstrings', 12, 65535, 0), ('calves', 12, 65535, 0),
    ('lower_back', 13, 65535, 1), ('glutes', 13, 65535, 0), ('hamstrings', 13, 65535, 0), ('abs', 16, 65535, 1),
    ('hips', 16, 65535, 1), ('quads', 16, 65535, 0), ('shoulders', 25, 65535, 1), ('traps', 26, 65535, 1),
    ('shoulders', 26, 65535, 0), ('shoulders', 38, 65535, 1), ('abs', 46, 65535, 1), ('lats', 46, 65535, 1),
    ('forearm', 46, 65535, 1), ('biceps', 46, 65535, 0), ('triceps', 46, 65535, 0), ('traps', 46, 65535, 0),
    ('shoulders', 46, 65535, 0), ('biceps', 50, 65535, 1), ('glutes', 50, 65535, 1), ('hamstrings', 50, 65535, 1),
    ('quads', 50, 65535, 1), ('hips', 50, 65535, 1), ('abs', 50, 65535, 0), ('chest', 50, 65535, 0),
    ('lats', 50, 65535, 0), ('lower_back', 50, 65535, 0), ('shoulders', 50, 65535, 0), ('traps', 50, 65535, 0),
    ('triceps', 50, 65535, 0);

-- Generic hyperextensions used to be parsed as 13/1 (weighted back extension with opposite arm and
-- leg reach) and shown as "Hyperextension": move them, with their settings, to the generic
UPDATE EXERCISE SET
    unilateral  = (SELECT unilateral FROM EXERCISE WHERE category = 13 AND id = 1),
    progression = (SELECT progression FROM EXERCISE WHERE category = 13 AND id = 1),
    reps        = (SELECT reps FROM EXERCISE WHERE category = 13 AND id = 1),
    weight      = (SELECT weight FROM EXERCISE WHERE category = 13 AND id = 1),
    notes       = (SELECT notes FROM EXERCISE WHERE category = 13 AND id = 1)
WHERE category = 13 AND id = 65535;

-- Workouts sent with the old exercise have to be sent again
UPDATE WORKOUT_DEVICE SET pending = TRUE
WHERE workout IN (SELECT workout FROM WORKOUT_STEP WHERE exercise_category = 13 AND exercise_id = 1);

UPDATE WORKOUT_STEP SET exercise_id = 65535 WHERE exercise_category = 13 AND exercise_id = 1;
UPDATE SESSION_STEP SET exercise_id = 65535 WHERE exercise_category = 13 AND exercise_id = 1;

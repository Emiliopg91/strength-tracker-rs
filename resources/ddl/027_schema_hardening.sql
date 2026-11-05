--- Harden the schema: nullable session_step reps/weight/duration (NULL reps and weight on rests), workout_step time in seconds, integer types, NOT NULLs and RESTRICT from catalogs to user data

-- Foreign keys stay on during migrations, so dropping a parent would cascade over its children:
-- rows are staged without foreign keys and tables dropped from children to parents
CREATE TABLE MIG_SUB_SPORT AS SELECT * FROM SUB_SPORT;
CREATE TABLE MIG_EXERCISE AS SELECT * FROM EXERCISE;
CREATE TABLE MIG_MUSCLE AS SELECT * FROM MUSCLE;
CREATE TABLE MIG_MUSCLE_EXERCISE AS SELECT * FROM MUSCLE_EXERCISE;
CREATE TABLE MIG_WORKOUT AS SELECT * FROM WORKOUT;
CREATE TABLE MIG_WORKOUT_STEP AS SELECT * FROM WORKOUT_STEP;
CREATE TABLE MIG_WORKOUT_DEVICE AS SELECT * FROM WORKOUT_DEVICE;
CREATE TABLE MIG_SESSION AS SELECT * FROM SESSION;
CREATE TABLE MIG_ADDITIONAL_DATA AS SELECT * FROM ADDITIONAL_DATA;
CREATE TABLE MIG_LAP AS SELECT * FROM LAP;
CREATE TABLE MIG_SESSION_STEP AS SELECT * FROM SESSION_STEP;

DROP TABLE SESSION_STEP;
DROP TABLE LAP;
DROP TABLE ADDITIONAL_DATA;
DROP TABLE SESSION;
DROP TABLE SUB_SPORT;
DROP TABLE WORKOUT_DEVICE;
DROP TABLE WORKOUT_STEP;
DROP TABLE WORKOUT;
DROP TABLE MUSCLE_EXERCISE;
DROP TABLE MUSCLE;
DROP TABLE EXERCISE;

CREATE TABLE SUB_SPORT (
    sport INTEGER NOT NULL,
    id    INTEGER NOT NULL,

    PRIMARY KEY (sport, id),
    FOREIGN KEY (sport) REFERENCES SPORT(id) ON DELETE RESTRICT
);

CREATE TABLE EXERCISE (
    category    INTEGER NOT NULL,
    id          INTEGER NOT NULL,
    name        TEXT NOT NULL DEFAULT '',
    unilateral  BOOLEAN NOT NULL DEFAULT FALSE,
    progression BOOLEAN NOT NULL DEFAULT TRUE,
    reps        TEXT NOT NULL DEFAULT '8-10-12',
    weight      REAL NOT NULL DEFAULT 2,
    notes       TEXT CHECK (length(notes) <= 200),

    PRIMARY KEY (category, id),
    FOREIGN KEY (category) REFERENCES EXERCISE_CATEGORY(id) ON DELETE RESTRICT
);

CREATE TABLE MUSCLE (
    name TEXT NOT NULL PRIMARY KEY
);

CREATE TABLE MUSCLE_EXERCISE (
    muscle            TEXT NOT NULL,
    exercise_category INTEGER NOT NULL,
    exercise_id       INTEGER NOT NULL,
    is_primary        INTEGER NOT NULL,

    PRIMARY KEY (muscle, exercise_category, exercise_id),
    FOREIGN KEY (muscle) REFERENCES MUSCLE(name) ON DELETE CASCADE,
    FOREIGN KEY (exercise_category, exercise_id) REFERENCES EXERCISE(category, id) ON DELETE CASCADE
);

CREATE TABLE WORKOUT (
    name       TEXT NOT NULL PRIMARY KEY,
    enabled    BOOLEAN NOT NULL DEFAULT TRUE,
    notes      TEXT CHECK (length(notes) <= 200),
    connect_id TEXT UNIQUE
);

CREATE TABLE WORKOUT_STEP (
    workout           TEXT NOT NULL,
    idx               INTEGER NOT NULL,
    kind              TEXT NOT NULL,
    exercise_category INTEGER,
    exercise_id       INTEGER,
    reps              INTEGER,
    weight            REAL,
    time              INTEGER,
    begin_idx         INTEGER,
    notes             TEXT CHECK (length(notes) <= 200),

    PRIMARY KEY (workout, idx),
    FOREIGN KEY (workout) REFERENCES WORKOUT(name) ON DELETE CASCADE,
    FOREIGN KEY (exercise_category, exercise_id) REFERENCES EXERCISE(category, id) ON DELETE RESTRICT
);

CREATE TABLE WORKOUT_DEVICE (
    workout TEXT NOT NULL,
    device  TEXT NOT NULL,
    pending BOOLEAN NOT NULL DEFAULT TRUE,

    PRIMARY KEY (workout, device),
    FOREIGN KEY (workout) REFERENCES WORKOUT(name),
    FOREIGN KEY (device) REFERENCES DEVICE(serial)
);

CREATE TABLE SESSION (
    date                 INTEGER NOT NULL PRIMARY KEY,
    name                 TEXT NOT NULL,
    total_elapsed_time   INTEGER NOT NULL,
    active_time          INTEGER NOT NULL DEFAULT 0,
    active_calories      INTEGER NOT NULL DEFAULT 0,
    training_load        INTEGER NOT NULL DEFAULT 0,
    device               TEXT,
    workout              TEXT,
    sport                INTEGER NOT NULL,
    sub_sport            INTEGER NOT NULL,
    show_exercises_chart BOOLEAN NOT NULL DEFAULT TRUE,

    FOREIGN KEY (sport, sub_sport) REFERENCES SUB_SPORT(sport, id) ON DELETE RESTRICT
);

CREATE TABLE ADDITIONAL_DATA (
    session     INTEGER NOT NULL PRIMARY KEY,
    heart_rates BLOB,
    coordinates BLOB,
    speeds      BLOB,
    distance    REAL,
    notes       TEXT,
    max_hr      INTEGER,

    FOREIGN KEY (session) REFERENCES SESSION(date) ON DELETE CASCADE
);

CREATE TABLE LAP (
    session         INTEGER NOT NULL,
    idx             INTEGER NOT NULL,
    start_latitude  INTEGER,
    start_longitude INTEGER,

    PRIMARY KEY (session, idx),
    FOREIGN KEY (session) REFERENCES SESSION(date) ON DELETE CASCADE
);

CREATE TABLE SESSION_STEP (
    session           INTEGER NOT NULL,
    idx               INTEGER NOT NULL,
    kind              TEXT NOT NULL,
    duration          INTEGER,
    duration_ref      INTEGER,
    exercise_category INTEGER,
    exercise_id       INTEGER,
    reps              INTEGER,
    reps_ref          INTEGER,
    weight            REAL,
    weight_ref        REAL,
    pr                BOOLEAN NOT NULL,

    PRIMARY KEY (session, idx),
    FOREIGN KEY (session) REFERENCES SESSION(date) ON DELETE CASCADE,
    FOREIGN KEY (exercise_category, exercise_id) REFERENCES EXERCISE(category, id) ON DELETE RESTRICT
);

INSERT INTO SUB_SPORT (sport, id) SELECT sport, id FROM MIG_SUB_SPORT;

INSERT INTO EXERCISE (category, id, name, unilateral, progression, reps, weight, notes)
SELECT category, id, name, unilateral, progression, reps, weight, notes FROM MIG_EXERCISE;

INSERT INTO MUSCLE (name) SELECT name FROM MIG_MUSCLE;

INSERT INTO MUSCLE_EXERCISE (muscle, exercise_category, exercise_id, is_primary)
SELECT muscle, exercise_category, exercise_id, is_primary FROM MIG_MUSCLE_EXERCISE;

INSERT INTO WORKOUT (name, enabled, notes, connect_id)
SELECT name, enabled, notes, connect_id FROM MIG_WORKOUT;

INSERT INTO WORKOUT_STEP (workout, idx, kind, exercise_category, exercise_id, reps, weight, time, begin_idx, notes)
SELECT workout, idx, kind, exercise_category, exercise_id, reps, weight, time, begin_idx, notes FROM MIG_WORKOUT_STEP;

INSERT INTO WORKOUT_DEVICE (workout, device, pending)
SELECT workout, device, pending FROM MIG_WORKOUT_DEVICE;

INSERT INTO SESSION (date, name, total_elapsed_time, active_time, active_calories, training_load, device, workout, sport, sub_sport, show_exercises_chart)
SELECT date, name, total_elapsed_time, active_time, active_calories, training_load, device, workout, sport, sub_sport, show_exercises_chart FROM MIG_SESSION;

INSERT INTO ADDITIONAL_DATA (session, heart_rates, coordinates, speeds, distance, notes, max_hr)
SELECT session, heart_rates, coordinates, speeds, distance, notes, max_hr FROM MIG_ADDITIONAL_DATA;

INSERT INTO LAP (session, idx, start_latitude, start_longitude)
SELECT session, idx, start_latitude, start_longitude FROM MIG_LAP;

INSERT INTO SESSION_STEP (session, idx, kind, duration, duration_ref, exercise_category, exercise_id, reps, reps_ref, weight, weight_ref, pr)
SELECT session, idx, kind, duration, duration_ref, exercise_category, exercise_id, reps, reps_ref, weight, weight_ref, pr FROM MIG_SESSION_STEP;

DROP TABLE MIG_SESSION_STEP;
DROP TABLE MIG_LAP;
DROP TABLE MIG_ADDITIONAL_DATA;
DROP TABLE MIG_SESSION;
DROP TABLE MIG_WORKOUT_DEVICE;
DROP TABLE MIG_WORKOUT_STEP;
DROP TABLE MIG_WORKOUT;
DROP TABLE MIG_MUSCLE_EXERCISE;
DROP TABLE MIG_MUSCLE;
DROP TABLE MIG_EXERCISE;
DROP TABLE MIG_SUB_SPORT;

CREATE INDEX SESSION_NAME ON SESSION(name);
CREATE INDEX WORKOUT_STEP_WORKOUT ON WORKOUT_STEP(workout);
CREATE INDEX WORKOUT_STEP_EXERCISE ON WORKOUT_STEP(exercise_category, exercise_id);
CREATE INDEX WORKOUT_DEVICE_WORKOUT ON WORKOUT_DEVICE(workout);
CREATE INDEX WORKOUT_DEVICE_DEVICE ON WORKOUT_DEVICE(device);
CREATE INDEX SESSION_STEP_EXERCISE ON SESSION_STEP(exercise_category, exercise_id);
CREATE INDEX SESSION_STEP_PR ON SESSION_STEP(pr);
CREATE UNIQUE INDEX SESSION_STEP_EXERCISE_PR ON SESSION_STEP(exercise_category, exercise_id) WHERE pr = 1;
CREATE INDEX SESSION_STEP_SESSION_PR ON SESSION_STEP(session, pr);

UPDATE session_step SET weight_ref=NULL, reps_ref=NULL WHERE kind="Rest";
UPDATE session_step SET duration_ref=NULL WHERE kind="Exercise";
UPDATE session_step SET reps = NULL, weight = NULL WHERE kind = 'Rest';
UPDATE workout_step SET time = time / 1000 WHERE time IS NOT NULL;

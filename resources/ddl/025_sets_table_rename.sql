--- Remove non-strength exercises and replace set table with session_step

DELETE FROM exercise WHERE (category,id) NOT IN (SELECT DISTINCT exercise_category, exercise_id FROM muscle_exercise) AND category!=65535;
DELETE FROM exercise_category WHERE id NOT IN (SELECT DISTINCT category FROM exercise);

CREATE TABLE SESSION_STEP (
    session           INTEGER NOT NULL,
    idx               INTEGER NOT NULL,
    exercise_category INTEGER,
    exercise_id       INTEGER,
    reps              INTEGER NOT NULL,
    reps_ref          NUMBER,
    weight            REAL NOT NULL,
    weight_ref        REAL,
    pr                BOOLEAN NOT NULL,
    duration          NUMBER NOT NULL DEFAULT 0,
    duration_ref      NUMBER,
    rest              BOOLEAN NOT NULL DEFAULT FALSE,

    PRIMARY KEY (session, idx),
    FOREIGN KEY (session) REFERENCES SESSION(date) ON DELETE CASCADE,
    FOREIGN KEY (exercise_category, exercise_id) REFERENCES EXERCISE(category, id) ON DELETE CASCADE
);

INSERT INTO SESSION_STEP (session, idx, exercise_category, exercise_id, reps, weight, pr, duration, rest)
SELECT session, idx, exercise_category, exercise_id, reps, weight, pr, duration, exercise_category = 65535
FROM "SET";

UPDATE SESSION_STEP SET exercise_category=NULL, exercise_id=NULL WHERE exercise_category=65535;

DROP TABLE "SET";

CREATE INDEX SESSION_STEP_EXERCISE ON SESSION_STEP(exercise_category, exercise_id);
CREATE INDEX SESSION_STEP_PR ON SESSION_STEP(pr);
CREATE UNIQUE INDEX SESSION_STEP_EXERCISE_PR ON SESSION_STEP(exercise_category, exercise_id) WHERE pr = 1;
CREATE INDEX SESSION_STEP_SESSION_PR ON SESSION_STEP(session, pr);

DELETE FROM EXERCISE_CATEGORY WHERE id=65535;
DELETE FROM EXERCISE WHERE category=65535;
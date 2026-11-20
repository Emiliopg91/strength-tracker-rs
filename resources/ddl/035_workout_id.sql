--- Workouts keyed by an autoincrement id instead of their name, which stays unique; their steps, devices, plans and sessions reference that id

-- Foreign keys stay on during migrations, so dropping WORKOUT would cascade over its children:
-- rows are staged without foreign keys and tables dropped from children to parents
CREATE TABLE MIG_WORKOUT AS SELECT * FROM WORKOUT;
CREATE TABLE MIG_WORKOUT_STEP AS SELECT * FROM WORKOUT_STEP;
CREATE TABLE MIG_WORKOUT_DEVICE AS SELECT * FROM WORKOUT_DEVICE;
CREATE TABLE MIG_WORKOUT_PLANNING AS SELECT * FROM WORKOUT_PLANNING;

DROP TABLE WORKOUT_PLANNING;
DROP TABLE WORKOUT_DEVICE;
DROP TABLE WORKOUT_STEP;
DROP TABLE WORKOUT;

CREATE TABLE WORKOUT (
    id                   INTEGER PRIMARY KEY AUTOINCREMENT,
    name                 TEXT NOT NULL,
    enabled              BOOLEAN NOT NULL DEFAULT TRUE,
    notes                TEXT CHECK (length(notes) <= 200),
    connect_id           TEXT UNIQUE,
    connect_created_time INTEGER,

    -- Mapped to a u16
    CHECK (id <= 65535)
);

CREATE UNIQUE INDEX WORKOUT_NAME ON WORKOUT(name);

CREATE TABLE WORKOUT_STEP (
    workout           INTEGER NOT NULL,
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
    FOREIGN KEY (workout) REFERENCES WORKOUT(id) ON DELETE CASCADE,
    FOREIGN KEY (exercise_category, exercise_id) REFERENCES EXERCISE(category, id) ON DELETE RESTRICT
);

CREATE TABLE WORKOUT_DEVICE (
    workout INTEGER NOT NULL,
    device  TEXT NOT NULL,
    pending BOOLEAN NOT NULL DEFAULT TRUE,

    PRIMARY KEY (workout, device),
    FOREIGN KEY (workout) REFERENCES WORKOUT(id) ON DELETE CASCADE,
    FOREIGN KEY (device) REFERENCES DEVICE(serial)
);

CREATE TABLE WORKOUT_PLANNING (
    date       INTEGER NOT NULL,
    workout    INTEGER NOT NULL,
    session    INTEGER,
    connect_id TEXT,

    PRIMARY KEY (date, workout),
    FOREIGN KEY (workout) REFERENCES WORKOUT(id) ON DELETE CASCADE,
    FOREIGN KEY (session) REFERENCES SESSION(date) ON DELETE SET NULL
);

-- Ids follow the alphabetical order of the existing workouts
INSERT INTO WORKOUT (name, enabled, notes, connect_id, connect_created_time)
SELECT name, enabled, notes, connect_id, connect_created_time FROM MIG_WORKOUT ORDER BY name;

-- Sessions can name a workout no longer stored: it is created to keep the link, disabled so it
-- doesn't come back to the list of active workouts
INSERT INTO WORKOUT (name, enabled)
SELECT DISTINCT workout, FALSE FROM SESSION
WHERE workout IS NOT NULL AND workout NOT IN (SELECT name FROM WORKOUT)
ORDER BY workout;

INSERT INTO WORKOUT_STEP (workout, idx, kind, exercise_category, exercise_id, reps, weight, time, begin_idx, notes)
SELECT w.id, s.idx, s.kind, s.exercise_category, s.exercise_id, s.reps, s.weight, s.time, s.begin_idx, s.notes
FROM MIG_WORKOUT_STEP s JOIN WORKOUT w ON w.name = s.workout;

INSERT INTO WORKOUT_DEVICE (workout, device, pending)
SELECT w.id, d.device, d.pending
FROM MIG_WORKOUT_DEVICE d JOIN WORKOUT w ON w.name = d.workout;

INSERT INTO WORKOUT_PLANNING (date, workout, session, connect_id)
SELECT p.date, w.id, p.session, p.connect_id
FROM MIG_WORKOUT_PLANNING p JOIN WORKOUT w ON w.name = p.workout;

DROP TABLE MIG_WORKOUT_PLANNING;
DROP TABLE MIG_WORKOUT_DEVICE;
DROP TABLE MIG_WORKOUT_STEP;
DROP TABLE MIG_WORKOUT;

CREATE INDEX WORKOUT_STEP_EXERCISE ON WORKOUT_STEP(exercise_category, exercise_id);
CREATE INDEX WORKOUT_DEVICE_DEVICE ON WORKOUT_DEVICE(device);
CREATE INDEX WORKOUT_PLANNING_SESSION ON WORKOUT_PLANNING(session);

-- SESSION keeps its children: the column is swapped in place instead of rebuilding the table
ALTER TABLE SESSION ADD COLUMN workout_id INTEGER REFERENCES WORKOUT(id) ON DELETE SET NULL;
UPDATE SESSION SET workout_id = (SELECT id FROM WORKOUT WHERE WORKOUT.name = SESSION.workout);
ALTER TABLE SESSION DROP COLUMN workout;
ALTER TABLE SESSION RENAME COLUMN workout_id TO workout;

-- Lets the SET NULL from WORKOUT, and the lookups of the sessions of a workout, use an index
CREATE INDEX SESSION_WORKOUT ON SESSION(workout);

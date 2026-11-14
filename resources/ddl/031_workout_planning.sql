--- Workout planning

CREATE TABLE WORKOUT_PLANNING (
    date INTEGER NOT NULL,
    workout TEXT NOT NULL,
    session INTEGER,
    connect_id TEXT,

    PRIMARY KEY (date, workout),
    FOREIGN KEY (workout) REFERENCES WORKOUT(name) ON DELETE CASCADE,
    FOREIGN KEY (session) REFERENCES SESSION(date) ON DELETE SET NULL
);

-- Lets the SET NULL from SESSION find its rows without a full scan
CREATE INDEX WORKOUT_PLANNING_SESSION ON WORKOUT_PLANNING(session);

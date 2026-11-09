--- Drop the never edited session.show_exercises_chart flag and cascade workout deletions to workout_device

ALTER TABLE SESSION DROP COLUMN show_exercises_chart;

-- Nothing references WORKOUT_DEVICE, so it can be dropped and rebuilt with foreign keys on
CREATE TABLE MIG_WORKOUT_DEVICE AS SELECT * FROM WORKOUT_DEVICE;
DROP TABLE WORKOUT_DEVICE;

CREATE TABLE WORKOUT_DEVICE (
    workout TEXT NOT NULL,
    device  TEXT NOT NULL,
    pending BOOLEAN NOT NULL DEFAULT TRUE,

    PRIMARY KEY (workout, device),
    FOREIGN KEY (workout) REFERENCES WORKOUT(name) ON DELETE CASCADE,
    FOREIGN KEY (device) REFERENCES DEVICE(serial)
);

INSERT INTO WORKOUT_DEVICE (workout, device, pending)
SELECT workout, device, pending FROM MIG_WORKOUT_DEVICE;
DROP TABLE MIG_WORKOUT_DEVICE;

-- Lookups by workout are covered by the primary key, so only the device index is recreated
CREATE INDEX WORKOUT_DEVICE_DEVICE ON WORKOUT_DEVICE(device);

ALTER TABLE LAP RENAME TO SESSION_LAP;
ALTER TABLE ADDITIONAL_DATA RENAME TO SESSION_DATA;

ALTER TABLE SESSION ADD COLUMN connect_id TEXT DEFAULT NULL;
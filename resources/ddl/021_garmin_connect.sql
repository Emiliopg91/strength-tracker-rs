--- Garmin connect

ALTER TABLE device ADD COLUMN last_body_metric_sync INTEGER;
ALTER TABLE device RENAME COLUMN last_sync TO last_activity_sync;

INSERT INTO device VALUES ('connect', 'Connect', NULL, NULL);
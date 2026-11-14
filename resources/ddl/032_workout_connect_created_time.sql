--- Creation time of the workout in Connect

-- UTC timestamp in seconds. Devices key workout and schedule files by (Connect id, creation time)
ALTER TABLE WORKOUT ADD COLUMN connect_created_time INTEGER;

-- Workout files now carry Garmin/Connect as maker, as schedules point to them by those keys, so the
-- ones already on devices are stale and get uploaded again
UPDATE WORKOUT_DEVICE SET pending = TRUE WHERE device <> 'connect';

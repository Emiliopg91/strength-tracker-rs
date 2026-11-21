--- Name a renamed workout still has on each device, so its old file is deleted before the next push

-- Device files are named after the workout, and a device ignores a workout with the serial and
-- creation time of one it already holds, so the old file must go before the renamed one lands
ALTER TABLE WORKOUT_DEVICE ADD COLUMN previous_name TEXT;

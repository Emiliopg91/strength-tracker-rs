--- Session new fields

ALTER TABLE session ADD COLUMN active_calories INTEGER NOT NULL DEFAULT 0;
UPDATE session SET active_calories=total_calories-metabolic_calories;
ALTER TABLE session DROP COLUMN total_calories;
ALTER TABLE session DROP COLUMN metabolic_calories;

DELETE FROM settings where name='auto_sync';
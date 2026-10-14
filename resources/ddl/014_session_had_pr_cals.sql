--- Session new fields

ALTER TABLE session ADD COLUMN active_calories BOOLEAN NOT NULL DEFAULT false;
UPDATE session SET active_calories=total_calories-metabolic_calories;
ALTER TABLE session DROP COLUMN total_calories;
ALTER TABLE session DROP COLUMN metabolic_calories;

DELETE FROM settings where name='auto_sync';
--- Customizable reps range per exercise

ALTER TABLE exercise ADD COLUMN reps TEXT NOT NULL DEFAULT "8-10-12";
UPDATE exercise SET reps = min_range || '-' || ((min_range + max_range) / 2) || '-' || max_range;

ALTER TABLE exercise DROP COLUMN min_range;
ALTER TABLE exercise DROP COLUMN max_range;
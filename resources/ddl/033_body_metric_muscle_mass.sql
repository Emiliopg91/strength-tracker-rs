--- Body metric lean mass renamed to muscle mass

-- The value always came from the scale's skeletal muscle mass, not from lean body mass
ALTER TABLE BODY_METRIC RENAME COLUMN lean_mass TO muscle_mass;

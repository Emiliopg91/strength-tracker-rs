--- Exercise steps loaded with a percentage of the current estimated 1RM

-- When set, WEIGHT holds the load it resolves to, refreshed whenever the estimated 1RM changes
ALTER TABLE WORKOUT_STEP ADD COLUMN one_rm_pct INTEGER CHECK (one_rm_pct BETWEEN 1 AND 100);

--- Show chart column for sessions
ALTER TABLE session ADD COLUMN show_exercises_chart BOOLEAN NOT NULL DEFAULT true;
ALTER TABLE exercise ADD COLUMN notes TEXT CHECK (length(notes) <= 200);
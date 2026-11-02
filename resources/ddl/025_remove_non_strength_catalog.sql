--- Remove non strength exercises

DELETE FROM exercise WHERE (category,id) NOT IN (SELECT DISTINCT exercise_category, exercise_id FROM muscle_exercise) AND category!=65535;
DELETE FROM exercise_category WHERE id NOT IN (SELECT DISTINCT category FROM exercise);

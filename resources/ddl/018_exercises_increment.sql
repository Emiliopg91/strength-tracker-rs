--- Customizable weight increment per exercise

ALTER TABLE exercise ADD COLUMN weight REAL NOT NULL DEFAULT 2 ;
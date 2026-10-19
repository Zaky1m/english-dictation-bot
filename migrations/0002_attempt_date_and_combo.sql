ALTER TABLE users ADD COLUMN combo INT NOT NULL DEFAULT 0;
-- Cairo calendar date the attempt belongs to (daily challenge and usage).
ALTER TABLE attempts ADD COLUMN cairo_date DATE;
ALTER TABLE challenge_scores ADD COLUMN solved INT NOT NULL DEFAULT 0;

-- Lifetime counters for the accuracy statistic; attempts are purged after a day.
ALTER TABLE users ADD COLUMN stat_solved INT NOT NULL DEFAULT 0;
ALTER TABLE users ADD COLUMN stat_clean INT NOT NULL DEFAULT 0;

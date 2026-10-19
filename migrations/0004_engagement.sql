-- A user who blocked the bot is not banned from the app; the two flags differ.
ALTER TABLE users ADD COLUMN bot_blocked BOOLEAN NOT NULL DEFAULT false;
ALTER TABLE progress ADD COLUMN clean BOOLEAN NOT NULL DEFAULT false;

CREATE TABLE daily_stats (
    user_id    BIGINT NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    cairo_date DATE NOT NULL,
    sentences  INT NOT NULL DEFAULT 0,
    points     INT NOT NULL DEFAULT 0,
    PRIMARY KEY (user_id, cairo_date)
);

CREATE TABLE missed_letters (
    user_id BIGINT NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    letter  CHAR(1) NOT NULL,
    count   INT NOT NULL DEFAULT 0,
    PRIMARY KEY (user_id, letter)
);

CREATE TABLE missed_words (
    user_id BIGINT NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    word    TEXT NOT NULL,
    count   INT NOT NULL DEFAULT 0,
    PRIMARY KEY (user_id, word)
);

-- One reminder per user per Cairo day, even across restarts.
CREATE TABLE reminder_log (
    user_id    BIGINT NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    cairo_date DATE NOT NULL,
    PRIMARY KEY (user_id, cairo_date)
);

ALTER TABLE daily_challenges ADD COLUMN posted_at TIMESTAMPTZ;

-- Both scheduled messages stay off until the maintainer approves the wording.
INSERT INTO settings (key, value) VALUES
    ('reminders_enabled', 'false'),
    ('challenge_post_enabled', 'false')
ON CONFLICT (key) DO NOTHING;

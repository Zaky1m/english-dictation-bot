CREATE TABLE users (
    id            BIGINT PRIMARY KEY,
    name          TEXT NOT NULL DEFAULT '',
    username      TEXT,
    lang          TEXT NOT NULL DEFAULT 'ar' CHECK (lang IN ('ar', 'en')),
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    blocked       BOOLEAN NOT NULL DEFAULT false,
    reminders_on  BOOLEAN NOT NULL DEFAULT true
);

CREATE TABLE settings (
    key        TEXT PRIMARY KEY,
    value      JSONB NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

INSERT INTO settings (key, value) VALUES
    ('free_daily_sentences', '10'),
    ('ad_unlocks_per_day', '1'),
    ('ad_extra_sentences', '10'),
    ('pro_daily_sentences', '50'),
    ('streak_goal', '10'),
    ('streak_freezes_per_month', '2'),
    ('reminder_hour', '20'),
    ('challenge_size', '5'),
    ('challenge_repeat_days', '30'),
    ('challenge_post_time', '"23:30"'),
    ('review_intervals_days', '[1, 3, 7]'),
    ('league_week_start', '"saturday"'),
    ('league_group_size', '30'),
    ('league_promote_top', '7'),
    ('league_demote_bottom', '5'),
    ('price_month_egp', '50'),
    ('price_month_usd', '2'),
    ('price_year_egp', '500'),
    ('price_year_usd', '20'),
    ('stars_month', '150'),
    ('stars_year', '1500'),
    ('stars_month_period_secs', '2592000'),
    ('ads_enabled', 'false'),
    ('ad_credit_ttl_minutes', '10'),
    ('score_base_constant', '10'),
    ('score_mistake_penalty', '0.10'),
    ('score_reveal_penalty', '0.25'),
    ('score_min_accuracy', '0.3'),
    ('score_expected_base_ms', '4000'),
    ('score_expected_per_letter_ms', '450'),
    ('score_speed_bonus_factor', '0.5'),
    ('score_combo_step', '0.1'),
    ('score_combo_cap', '5'),
    ('score_min_ms_per_letter', '80'),
    ('check_min_interval_ms', '250');

CREATE TABLE situations (
    id       SERIAL PRIMARY KEY,
    slug     TEXT NOT NULL UNIQUE,
    name_ar  TEXT NOT NULL,
    name_en  TEXT NOT NULL,
    icon     TEXT NOT NULL,
    position SMALLINT NOT NULL,
    active   BOOLEAN NOT NULL DEFAULT true
);

CREATE TABLE lessons (
    id           SERIAL PRIMARY KEY,
    situation_id INT NOT NULL REFERENCES situations (id) ON DELETE CASCADE,
    number       SMALLINT NOT NULL,
    UNIQUE (situation_id, number)
);

CREATE TABLE sentences (
    id          SERIAL PRIMARY KEY,
    lesson_id   INT NOT NULL REFERENCES lessons (id) ON DELETE CASCADE,
    position    SMALLINT NOT NULL,
    text        TEXT NOT NULL,
    normalized  TEXT NOT NULL UNIQUE,
    translation TEXT NOT NULL,
    difficulty  SMALLINT NOT NULL,
    tokens      JSONB NOT NULL,
    audio_ready BOOLEAN NOT NULL DEFAULT false,
    active      BOOLEAN NOT NULL DEFAULT true,
    UNIQUE (lesson_id, position)
);

CREATE TABLE words (
    word        TEXT PRIMARY KEY,
    audio_ready BOOLEAN NOT NULL DEFAULT false
);

CREATE TABLE sentence_words (
    sentence_id INT NOT NULL REFERENCES sentences (id) ON DELETE CASCADE,
    idx         SMALLINT NOT NULL,
    word        TEXT NOT NULL REFERENCES words (word),
    meaning_ar  TEXT NOT NULL,
    PRIMARY KEY (sentence_id, idx)
);
CREATE INDEX sentence_words_word_idx ON sentence_words (word);

CREATE TABLE progress (
    user_id      BIGINT NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    sentence_id  INT NOT NULL REFERENCES sentences (id) ON DELETE CASCADE,
    best_score   INT NOT NULL,
    completed_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (user_id, sentence_id)
);

CREATE TABLE lesson_progress (
    user_id      BIGINT NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    lesson_id    INT NOT NULL REFERENCES lessons (id) ON DELETE CASCADE,
    completed_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (user_id, lesson_id)
);

-- Temporary: rows are purged 24 hours after they start.
CREATE TABLE attempts (
    id               BIGSERIAL PRIMARY KEY,
    user_id          BIGINT NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    sentence_id      INT NOT NULL REFERENCES sentences (id) ON DELETE CASCADE,
    kind             TEXT NOT NULL CHECK (kind IN ('lesson', 'review', 'challenge')),
    started_at       TIMESTAMPTZ NOT NULL DEFAULT now(),
    audio_started_at TIMESTAMPTZ,
    mistakes         JSONB NOT NULL DEFAULT '[]',
    reveals          INT[] NOT NULL DEFAULT '{}',
    last_check_at    TIMESTAMPTZ,
    completed_at     TIMESTAMPTZ,
    score            INT,
    flagged          BOOLEAN NOT NULL DEFAULT false
);
CREATE INDEX attempts_open_idx ON attempts (user_id, sentence_id) WHERE completed_at IS NULL;
CREATE INDEX attempts_started_idx ON attempts (started_at);

CREATE TABLE daily_usage (
    user_id    BIGINT NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    cairo_date DATE NOT NULL,
    started    INT NOT NULL DEFAULT 0,
    ad_unlocks INT NOT NULL DEFAULT 0,
    PRIMARY KEY (user_id, cairo_date)
);

CREATE TABLE streaks (
    user_id              BIGINT PRIMARY KEY REFERENCES users (id) ON DELETE CASCADE,
    current              INT NOT NULL DEFAULT 0,
    best                 INT NOT NULL DEFAULT 0,
    last_day             DATE,
    freezes_left         INT NOT NULL DEFAULT 0,
    freezes_reset_month  DATE
);

CREATE TABLE review_queue (
    user_id     BIGINT NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    sentence_id INT NOT NULL REFERENCES sentences (id) ON DELETE CASCADE,
    stage       SMALLINT NOT NULL,
    due_on      DATE NOT NULL,
    PRIMARY KEY (user_id, sentence_id)
);
CREATE INDEX review_queue_due_idx ON review_queue (user_id, due_on);

CREATE TABLE saved_words (
    user_id  BIGINT NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    word     TEXT NOT NULL REFERENCES words (word),
    saved_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (user_id, word)
);

CREATE TABLE user_badges (
    user_id   BIGINT NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    badge     TEXT NOT NULL,
    earned_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (user_id, badge)
);

CREATE TABLE league_weeks (
    week_start   DATE PRIMARY KEY,
    processed    BOOLEAN NOT NULL DEFAULT false,
    processed_at TIMESTAMPTZ
);

CREATE TABLE league_groups (
    id         BIGSERIAL PRIMARY KEY,
    week_start DATE NOT NULL REFERENCES league_weeks (week_start),
    tier       SMALLINT NOT NULL
);
CREATE INDEX league_groups_open_idx ON league_groups (week_start, tier);

CREATE TABLE league_members (
    group_id   BIGINT NOT NULL REFERENCES league_groups (id) ON DELETE CASCADE,
    week_start DATE NOT NULL,
    user_id    BIGINT NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    points     INT NOT NULL DEFAULT 0,
    PRIMARY KEY (group_id, user_id),
    UNIQUE (week_start, user_id)
);
CREATE INDEX league_members_rank_idx ON league_members (group_id, points DESC);

CREATE TABLE user_tiers (
    user_id BIGINT PRIMARY KEY REFERENCES users (id) ON DELETE CASCADE,
    tier    SMALLINT NOT NULL DEFAULT 0
);

CREATE TABLE daily_challenges (
    cairo_date   DATE PRIMARY KEY,
    sentence_ids INT[] NOT NULL
);

CREATE TABLE challenge_scores (
    cairo_date DATE NOT NULL REFERENCES daily_challenges (cairo_date),
    user_id    BIGINT NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    score      INT NOT NULL DEFAULT 0,
    PRIMARY KEY (cairo_date, user_id)
);
CREATE INDEX challenge_scores_rank_idx ON challenge_scores (cairo_date, score DESC);

CREATE TABLE subscriptions (
    id             BIGSERIAL PRIMARY KEY,
    user_id        BIGINT NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    plan           TEXT NOT NULL CHECK (plan IN ('month', 'year', 'custom')),
    source         TEXT NOT NULL CHECK (source IN ('stars', 'manual')),
    starts_at      TIMESTAMPTZ NOT NULL,
    ends_at        TIMESTAMPTZ NOT NULL,
    stars_charge_id TEXT UNIQUE,
    granted_by     BIGINT
);
CREATE INDEX subscriptions_user_idx ON subscriptions (user_id, ends_at DESC);

CREATE TABLE payments (
    telegram_payment_charge_id TEXT PRIMARY KEY,
    user_id    BIGINT NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    amount     INT NOT NULL,
    currency   TEXT NOT NULL,
    payload    TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX payments_user_idx ON payments (user_id, created_at DESC);

CREATE TABLE ad_credits (
    id          BIGSERIAL PRIMARY KEY,
    user_id     BIGINT NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    claimed_at  TIMESTAMPTZ,
    claimed_for TEXT CHECK (claimed_for IN ('reveal', 'extra'))
);
CREATE INDEX ad_credits_unclaimed_idx ON ad_credits (user_id, created_at) WHERE claimed_at IS NULL;

-- Temporary: webhook de-duplication, purged after 24 hours.
CREATE TABLE processed_updates (
    update_id  BIGINT PRIMARY KEY,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX processed_updates_created_idx ON processed_updates (created_at);

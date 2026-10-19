-- Long admin tasks (generate a situation, regenerate audio). One row per run,
-- so the panel can show progress and a restart can mark interrupted work.
CREATE TABLE content_jobs (
    id           BIGSERIAL PRIMARY KEY,
    kind         TEXT NOT NULL CHECK (kind IN ('generate', 'audio')),
    situation_id INT REFERENCES situations (id) ON DELETE SET NULL,
    status       TEXT NOT NULL CHECK (status IN ('running', 'done', 'failed')),
    detail       TEXT NOT NULL DEFAULT '',
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    finished_at  TIMESTAMPTZ
);
CREATE INDEX content_jobs_recent_idx ON content_jobs (id DESC);

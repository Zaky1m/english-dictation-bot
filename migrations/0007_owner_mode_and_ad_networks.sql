-- New installs start in development mode: only admins can use the bot and the
-- Mini App until an admin opens it to everyone.
INSERT INTO settings (key, value) VALUES
    ('public_access', 'false'),
    ('ad_provider_order', '["adsgram", "monetag", "adexium", "telega", "gigapub"]'),
    ('client_ad_credits_per_day', '20')
ON CONFLICT (key) DO NOTHING;

-- Networks other than Adsgram confirm a watched ad from the client only, so
-- their credits get a separate daily cap.
ALTER TABLE daily_usage ADD COLUMN client_ad_credits INT NOT NULL DEFAULT 0;

-- 'lessons' writes every situation from the built-in list, then its audio.
ALTER TABLE content_jobs DROP CONSTRAINT content_jobs_kind_check;
ALTER TABLE content_jobs ADD CONSTRAINT content_jobs_kind_check
    CHECK (kind IN ('generate', 'audio', 'lessons'));

-- Supported languages now come from web/i18n/languages.json and are checked by
-- the app, so adding one no longer needs a schema change.
ALTER TABLE users DROP CONSTRAINT IF EXISTS users_lang_check;

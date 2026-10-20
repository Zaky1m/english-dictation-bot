# Changelog

All notable changes are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

### Added
- Project skeleton: configuration loading and a `/health` endpoint.
- Gemini key pool, text-to-speech client and `eng content voices`.
- Initial database schema with seeded settings.
- Mini App authentication, scoring, answer checking and the learner API.
- Telegram bot: webhook with deduplication, start and language switch, Stars invoices and payments.
- Mini App: map, exercise (lesson, review, daily challenge), lesson result, daily limit, profile, saved words, Pro and settings screens.
- Streaks with Pro freezes, weekly leagues, badges, Pro statistics, daily challenge pick, purge, and scheduled reminder and channel jobs (off until approved).
- Adsgram rewarded ads (credits, extra sentences, ad-gated reveal) and the admin panel (users, Pro grants, payments, settings editor, content tools).
- Public repository files: English and Arabic READMEs, MIT license, contributing and security guides, Dockerfile, compose file and a systemd unit example.
- Interface languages are listed in `web/i18n/languages.json`; a new language needs only its text files, described in `TRANSLATING.md`.
- The server listens on `PORT` when `BIND_ADDR` is not set, for hosting platforms such as Railway.
- Development mode: a new install answers only the admins until an admin opens it to everyone.
- Owner tools in the bot: content counts, writing lessons, recording audio and the development-mode switch. Interrupted content tasks resume after a restart.
- Rewarded ads from Monetag, Adexium, Telega.io and GigaPub alongside Adsgram, tried in a configurable order.
- Guides for how the app works, Pro membership and deployment, in English and Arabic.
- Audio can be recorded with ElevenLabs instead of Gemini (`ELEVENLABS_API_KEY`, `ELEVENLABS_VOICE_ID`).
- MP3 files placed in `DATA_DIR/audio` by hand are picked up before recording, and `eng content audio --adopt` marks them without recording anything.
- The audio task pauses when the quota runs out and carries on when it comes back, instead of stopping.

### Fixed
- Admin panel layout on phones: icon tabs that fit the screen, one card per record with its actions below, a settings filter and switches for on and off settings.
- Saving a setting that holds a list of names, such as `ad_provider_order`, was rejected.
- Faster first load: every Mini App module is preloaded in parallel, the Telegram script no longer blocks the first paint, and a loader shows at once instead of a blank window.

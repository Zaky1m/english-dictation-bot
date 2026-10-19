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

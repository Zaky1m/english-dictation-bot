# eng

[العربية](README.ar.md)

A Telegram Mini App for learning English by dictation. The learner hears a
sentence, sees its Arabic translation, and types the English. A Rust backend
(axum and PostgreSQL) serves the bot webhook, the JSON API and the static Mini
App. Answers are checked on the server, so the client never holds the solution
before the sentence is solved or a word is revealed.

![The exercise screen on a phone, with one wrong letter underlined](docs/screenshots/exercise-mobile.png)

![The lesson map on a desktop window](docs/screenshots/map-desktop.png)

## Features

- 20 situations, 6 lessons each, 10 sentences per lesson (1200 sentences),
  written once with Gemini and checked by automated validation.
- Audio for every sentence and every word, generated once, stored as MP3 and
  served as static files.
- One input box per word. The server returns the wrong letter positions; the
  correct answer is sent only after the sentence is solved or a word is revealed.
- Server-side scoring with accuracy, a speed bonus, a combo and basic anti-cheat.
- Daily goal and streak, with monthly streak freezes for Pro users.
- Spaced review of sentences with mistakes (after 1, 3 and 7 days).
- Weekly leagues in seven tiers with promotion and demotion.
- A daily challenge with its own leaderboard.
- Badges, saved words, and statistics (charts and most-missed letters for Pro).
- Pro subscriptions paid with Telegram Stars, or granted manually by an admin.
- Optional rewarded ads through Adsgram.
- An admin panel for users, Pro grants, payments, settings and content.
- Arabic and English interface, right-to-left and left-to-right. Desktop windows
  get the full window and keyboard shortcuts.

## Requirements

- Rust (stable, edition 2024) and a PostgreSQL 16 database.
- `ffmpeg`, only to generate audio.
- A Telegram bot token from BotFather.
- Gemini API keys, only to generate content and audio.
- A public HTTPS URL for the webhook and the Mini App.

## Run it locally

```bash
cp .env.example .env          # fill in the values
cargo run --release -- serve
```

`serve` applies the database migrations, registers the webhook and starts the
HTTP server. Always build with `--release`.

Content is generated with the same binary:

```bash
cargo run --release -- content generate --all   # sentences for every situation
cargo run --release -- content audio            # missing sentence and word audio
```

With Docker, `compose.yml` starts the app and PostgreSQL together:

```bash
cp .env.example .env
docker compose up --build
```

## Configuration

Everything comes from environment variables (see `.env.example`):

| Variable | Purpose |
|---|---|
| `BOT_TOKEN` | Telegram bot token |
| `PUBLIC_URL` | Public HTTPS origin of the Mini App |
| `BIND_ADDR` | Address the server listens on |
| `DATABASE_URL` | PostgreSQL connection string |
| `WEBHOOK_PATH`, `WEBHOOK_SECRET` | Webhook path segment and secret header |
| `ADSGRAM_REWARD_SECRET`, `ADSGRAM_BLOCK_ID` | Rewarded ads; off while the block id is empty |
| `ADMIN_IDS`, `ADMIN_USERNAME` | Admin user ids; contact for manual payments |
| `CHALLENGE_CHANNEL_ID` | Channel for the daily challenge summary |
| `DATA_DIR`, `WEB_DIR` | Audio files and static Mini App directories |
| `GEMINI_API_KEY_<n>` | Gemini keys, used round-robin |

Numbers such as daily limits, prices and scoring coefficients live in the
`settings` table and are editable from the admin panel without a restart.

## Architecture

One binary, `eng`, with these subcommands: `serve`, `content generate`,
`content audio`, `content voices` and `webhook set`.

```
src/
  http/        axum router, authentication, one file per API area
  telegram/    webhook handlers, bot texts, small caps, payments
  exercise/    tokenizer, checker, scoring (pure functions)
  content/     sentence generation, validation, audio, admin tasks
  gemini/      key pool, text and speech clients, audio cleaning
  jobs.rs      scheduled work: leagues, daily challenge, purge, reminders
migrations/    sqlx migrations
web/           the Mini App (vanilla HTML, CSS and JS, no build step)
texts/         bot messages in Arabic and English
```

All durable state is in PostgreSQL. Temporary state (open attempts, update ids
for webhook de-duplication, unclaimed ad credits) is purged after at most a day.
The reasoning behind the main choices is in [docs/DECISIONS.md](docs/DECISIONS.md)
and the product behaviour is in [docs/SPEC.md](docs/SPEC.md).

## Tests

```bash
cargo fmt --all --check
cargo clippy --all-targets --release -- -D warnings
cargo test --release
```

Tests that need a database run when `TEST_DATABASE_URL` points at a disposable
PostgreSQL database; each test uses its own temporary schema and removes it.

## License

[MIT](LICENSE)

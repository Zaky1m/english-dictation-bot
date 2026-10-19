# English Dictation Bot

### Learn English by listening and typing

A Telegram Mini App for practicing English through dictation. You hear a sentence, type what you hear, and the app marks every wrong letter until you get it right. Lessons follow everyday situations, and a daily goal, weekly leagues and a daily challenge keep learners coming back.

[Quick start](#quick-start) · [Configuration](#configuration) · [Translations](#translations) · [Product spec](docs/SPEC.md) · [العربية](README.ar.md)

![The exercise screen on a phone, with one wrong letter underlined](docs/screenshots/exercise-mobile-en.png)

![The lesson map on a desktop window](docs/screenshots/map-desktop-en.png)

## What you get

- Short dictation exercises with recorded audio for every sentence and every word
- One input box per word, with wrong letters marked as you type
- A translation under each sentence and the meaning of each word
- Lessons grouped by everyday situations, from greetings to travel
- Answers checked on the server, so the solution never reaches the app early
- A daily goal and streak, spaced review of mistakes, weekly leagues and a daily challenge
- Badges, saved words and progress statistics
- Pro subscriptions paid with Telegram Stars, and optional rewarded ads
- An admin panel for users, payments, settings and lesson content
- Right-to-left and left-to-right layouts, with keyboard shortcuts on desktop

## Quick start

You need Docker, a Telegram bot token from [BotFather](https://t.me/BotFather) and a public HTTPS address. Telegram only calls HTTPS, so for a local try-out put the app behind a tunnel or a reverse proxy.

```bash
cp .env.example .env              # fill in BOT_TOKEN, PUBLIC_URL, POSTGRES_PASSWORD and the secrets
docker compose up --build         # starts the app and PostgreSQL
```

The app applies its database migrations and registers the webhook on start.

The repository ships the code, not the lessons. Generate them with your own Gemini API keys (`GEMINI_API_KEY_1`, `GEMINI_API_KEY_2` and so on in `.env`):

```bash
# write the sentences, translations and word meanings for every situation
docker compose run --rm app eng content generate --all

# record the missing sentence and word audio
docker compose run --rm app eng content audio
```

The free Gemini tier limits speech requests per key per day, so recording all audio can take several days. More keys, or a key with billing enabled, make it faster.

### Without Docker

You need Rust (stable, edition 2024), PostgreSQL 16 and `ffmpeg`.

```bash
cp .env.example .env              # set DATABASE_URL to your PostgreSQL database
cargo run --release -- serve      # always build with --release
cargo run --release -- content generate --all
cargo run --release -- content audio
```

## Configuration

Everything comes from environment variables, and `.env.example` lists all of them.

| Variable | Purpose |
|---|---|
| `BOT_TOKEN` | Telegram bot token |
| `PUBLIC_URL` | Public HTTPS origin of the Mini App |
| `BIND_ADDR` | Address the server listens on |
| `DATABASE_URL` | PostgreSQL connection string |
| `WEBHOOK_PATH`, `WEBHOOK_SECRET` | Webhook path segment and secret header |
| `ADSGRAM_REWARD_SECRET`, `ADSGRAM_BLOCK_ID` | Rewarded ads, off while the block id is empty |
| `ADMIN_IDS`, `ADMIN_USERNAME` | Admin Telegram ids, and the contact shown for manual payments |
| `CHALLENGE_CHANNEL_ID` | Channel that receives the daily challenge summary |
| `DATA_DIR`, `WEB_DIR` | Audio files and static Mini App directories |
| `GEMINI_API_KEY_<n>` | Gemini keys, used round-robin |

Daily limits, prices and scoring rules live in the `settings` table. The admin panel edits them and the change applies without a restart.

## Translations

The interface and the bot speak Arabic and English today. Adding a language takes two text files and one line in a list, with no code changes, and [TRANSLATING.md](TRANSLATING.md) walks through it. Fixes to existing translations are just as welcome.

The lesson content (sentence translations and word meanings) is generated in Arabic at the moment.

## How it is built

One Rust binary, `eng`, serves the bot webhook, the JSON API and the Mini App. Its subcommands are `serve`, `content generate`, `content audio`, `content voices` and `webhook set`.

```
src/
  http/        axum router, authentication, one file per API area
  telegram/    webhook handlers, bot texts, payments
  exercise/    tokenizer, checker, scoring (pure functions)
  content/     sentence generation, validation, audio, admin tasks
  gemini/      key pool, text and speech clients, audio cleaning
  jobs.rs      scheduled work: leagues, daily challenge, purge, reminders
migrations/    sqlx migrations
web/           the Mini App, plain HTML, CSS and JS with no build step
web/i18n/      Mini App strings and the language list
texts/         bot messages per language
```

All durable state is in PostgreSQL. Temporary state (open attempts, webhook update ids, unclaimed ad credits) is purged after at most a day. [docs/SPEC.md](docs/SPEC.md) describes the product behavior and [docs/DECISIONS.md](docs/DECISIONS.md) records why the main choices were made.

## Development

```bash
cargo fmt --all --check
cargo clippy --all-targets --release -- -D warnings
cargo test --release
```

Tests that need a database run when `TEST_DATABASE_URL` points at a disposable PostgreSQL database. Each test creates its own temporary schema and drops it afterwards.

## Help and contributing

Bugs and ideas go to the issue tracker. [CONTRIBUTING.md](CONTRIBUTING.md) lists the checks to run before a pull request, [TRANSLATING.md](TRANSLATING.md) covers translations, and security problems are reported as described in [SECURITY.md](SECURITY.md).

## License

Released under the [MIT License](LICENSE)

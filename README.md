# English Dictation Bot

[العربية](README.ar.md)

Learn English by typing what you hear. A Telegram Mini App for Arabic speakers, built with Rust and PostgreSQL.

![The exercise screen on a phone, with one wrong letter underlined](docs/screenshots/exercise-mobile-en.png)

![The lesson map on a desktop window](docs/screenshots/map-desktop-en.png)

Each exercise plays one English sentence and shows its Arabic translation. The learner types the sentence, one box per word. The server checks the answer and returns the positions of the wrong letters. It sends the correct sentence only after the learner solves it or reveals a word, so the client never holds the answer early.

## What is in it

- 20 everyday situations, 6 lessons each, 10 sentences per lesson. That is 1,200 sentences, generated once with Gemini and filtered by automated validation.
- Audio for every sentence and every word, stored as MP3 and served as static files.
- Scoring on the server: accuracy, a speed bonus, a combo and basic anti-cheat.
- A daily goal and streak. Pro users get two streak freezes a month.
- Spaced review of sentences the learner got wrong, after 1, 3 and 7 days.
- Weekly leagues in seven tiers, with promotion and demotion, and a daily challenge with its own leaderboard.
- Badges, saved words and statistics. Pro adds charts and the most-missed letters.
- Pro subscriptions paid with Telegram Stars, or granted by an admin.
- Optional rewarded ads through Adsgram.
- An admin panel for users, Pro grants, payments, settings and content.
- Arabic and English interface, right-to-left and left-to-right. On a desktop the Mini App fills the window and supports keyboard shortcuts.

## Quick start

You need Rust (stable, edition 2024), PostgreSQL 16 and a Telegram bot token from BotFather. Telegram only talks to public HTTPS addresses, so for local testing put the app behind a tunnel or a reverse proxy.

```bash
cp .env.example .env          # fill in the values
cargo run --release -- serve
```

`serve` applies the database migrations, registers the webhook and starts the HTTP server. Always build with `--release`.

With Docker, `compose.yml` starts the app and PostgreSQL together:

```bash
cp .env.example .env
docker compose up --build
```

The repository ships the code but not the content. To fill the database, run the generator with your own Gemini keys. Audio generation also needs `ffmpeg`.

```bash
cargo run --release -- content generate --all   # sentences for every situation
cargo run --release -- content audio            # missing sentence and word audio
```

The free Gemini tier limits speech requests per key per day, so generating all audio can take several days unless you add more keys or enable billing.

## Configuration

Everything comes from environment variables. `.env.example` lists them all.

| Variable | Purpose |
|---|---|
| `BOT_TOKEN` | Telegram bot token |
| `PUBLIC_URL` | Public HTTPS origin of the Mini App |
| `BIND_ADDR` | Address the server listens on |
| `DATABASE_URL` | PostgreSQL connection string |
| `WEBHOOK_PATH`, `WEBHOOK_SECRET` | Webhook path segment and secret header |
| `ADSGRAM_REWARD_SECRET`, `ADSGRAM_BLOCK_ID` | Rewarded ads, off while the block id is empty |
| `ADMIN_IDS`, `ADMIN_USERNAME` | Admin user ids, and the contact shown for manual payments |
| `CHALLENGE_CHANNEL_ID` | Channel that receives the daily challenge summary |
| `DATA_DIR`, `WEB_DIR` | Audio files and static Mini App directories |
| `GEMINI_API_KEY_<n>` | Gemini keys, used round-robin |

Daily limits, prices and scoring coefficients live in the `settings` table. The admin panel edits them and the change applies without a restart.

## How it is built

One binary, `eng`, with the subcommands `serve`, `content generate`, `content audio`, `content voices` and `webhook set`.

```
src/
  http/        axum router, authentication, one file per API area
  telegram/    webhook handlers, bot texts, small caps, payments
  exercise/    tokenizer, checker, scoring (pure functions)
  content/     sentence generation, validation, audio, admin tasks
  gemini/      key pool, text and speech clients, audio cleaning
  jobs.rs      scheduled work: leagues, daily challenge, purge, reminders
migrations/    sqlx migrations
web/           the Mini App: plain HTML, CSS and JS, no build step
texts/         bot messages in Arabic and English
```

All durable state is in PostgreSQL. Temporary state (open attempts, webhook update ids, unclaimed ad credits) is purged after at most a day. [docs/SPEC.md](docs/SPEC.md) describes the product behavior and [docs/DECISIONS.md](docs/DECISIONS.md) records why the main choices were made.

## Tests

```bash
cargo fmt --all --check
cargo clippy --all-targets --release -- -D warnings
cargo test --release
```

Tests that need a database run when `TEST_DATABASE_URL` points at a disposable PostgreSQL database. Each test creates its own temporary schema and drops it afterwards.

## Contributing

Issues and pull requests are welcome. [CONTRIBUTING.md](CONTRIBUTING.md) has the checks to run first. Report security problems as described in [SECURITY.md](SECURITY.md).

## License

[MIT](LICENSE)

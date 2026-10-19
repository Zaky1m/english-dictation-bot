# CLAUDE.md

Guidance for AI agents and contributors working on this repository.

## What this is

A Telegram Mini App for learning English through dictation. The learner hears a
sentence (TTS audio), sees its Arabic translation, and types the English. A Rust
backend (axum + PostgreSQL) serves the bot webhook, the JSON API, and the static
Mini App. Answers are checked on the server, so the client never holds the
solution before it is solved or revealed.

## Read before you change anything

1. `docs/SPEC.md`: product behaviour. This is the source of truth.
2. `docs/DECISIONS.md`: why things are the way they are. Append to it when you
   make a non-obvious choice.
3. `CLAUDE.local.md`, if present: deployment notes for one specific server. It
   is never committed.

## Commands

```bash
cargo fmt --all
cargo clippy --all-targets --release -- -D warnings
cargo test --release
cargo build --release
cargo run --release -- serve        # needs a filled .env (see .env.example)
```

Always build and test with `--release`. The dev profile fills the disk on a
shared server. Run `cargo clean --profile dev` if a debug build ever appears.

## Code rules

- Stable Rust, edition 2024. No `unsafe`.
- No `unwrap()` or `expect()` outside tests and startup config parsing. Return
  errors with context (`anyhow::Context`) in binaries and typed errors
  (`thiserror`) inside library modules.
- Comments are English, short, and say why, not what. Do not restate the code.
- Name things by domain meaning (`attempt`, `league_group`), never `utils`,
  `helper`, or `manager`.
- Keep modules small and cohesive. One concern per file. Prefer plain functions
  that are easy to unit-test over large structs with hidden state.
- All durable state lives in PostgreSQL. In-memory caches are allowed only for
  data that can be rebuilt from the database (settings, key cooldowns).
- Any money or reward action (payment, ad credit, Pro grant) is recorded
  atomically and idempotently in the database before it takes effect.
- Webhook only. Never call `getUpdates`.
- Log lines, errors and CLI output are English. User-facing text comes from
  the i18n files, never hard-coded in Rust or JS.
- No secrets in code, tests, fixtures, docs or commit messages. Configuration
  comes from environment variables listed in `.env.example`.

## Frontend rules

- Vanilla HTML, CSS and JS in `web/`. No build step and no framework.
- No emoji anywhere in the UI. Icons are inline SVG from `web/icons.js`.
- Learning content (sentences, words, typed input) always uses normal Latin
  letters, never decorative Unicode.

## Commits

- Small commits, each with one reason, in imperative mood
  (`Add league promotion job`). No giant "initial" dump of the whole project.
- Run fmt, clippy and tests before every commit.
- End commit messages with the `Co-Authored-By` line the maintainer uses.

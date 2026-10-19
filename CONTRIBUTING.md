# Contributing

Thanks for helping. Please read `docs/SPEC.md` first: it is the source of truth
for product behaviour, and changes to it need a discussion in an issue.

## Build and test

```bash
cargo fmt --all
cargo clippy --all-targets --release -- -D warnings
cargo test --release
```

Build with `--release`; debug builds are large. Tests that use PostgreSQL run
when `TEST_DATABASE_URL` is set to a disposable database (each test creates and
drops its own schema).

## Rules

- Stable Rust, edition 2024, no `unsafe`.
- No `unwrap()` or `expect()` outside tests and startup configuration parsing.
- Comments are in English, short, and explain why rather than what.
- Webhook only: never call `getUpdates`.
- Money and reward actions are recorded atomically and idempotently in the
  database before they take effect.
- User-facing text lives in `texts/` and `web/i18n/`, never in Rust or JS code.
  Translations follow [TRANSLATING.md](TRANSLATING.md).
- The Mini App is plain HTML, CSS and JS with no build step, no emoji, and
  inline SVG icons.
- No secrets, personal data or deployment details in code, tests, docs or
  commit messages.

## Commits

Small commits with one reason each, in the imperative mood ("Add league
promotion job"). Run the three commands above before every commit.

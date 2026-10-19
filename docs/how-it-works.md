# How it works

[العربية](how-it-works.ar.md) · [Back to the README](../README.md)

## A lesson from the learner's side

The home screen is a map of everyday situations. Each situation has six lessons of ten sentences, and the sentences get longer from one lesson to the next, from three words up to about sixteen. A lesson opens once the one before it is finished.

Inside a lesson every sentence follows the same steps:

1. The learner plays the sentence, as many times as they like, at normal speed or at 0.7×
2. The translation sits above the input boxes, one box per word, so the number and length of the words are a hint
3. Only the letters a to z can be typed. Punctuation and apostrophes are already printed in place
4. Half a second after the last key, the server checks the letters and underlines every wrong one
5. When every word is right the sentence completes by itself. There is no submit button
6. Each word of the solved sentence can be tapped to hear it again, read its meaning and save it to My words

A learner who is stuck can reveal the current word. Pro members reveal freely, and free users watch a short ad when ads are enabled.

## Why the answer stays on the server

The app never receives the correct sentence before it is solved. It sends the typed letters and gets back only the positions of the wrong ones, and a revealed word comes back alone. Reading the network traffic therefore gives nothing away, and scores stay fair in the leagues.

## Scoring

Every solved sentence earns points from three parts:

- A base that grows with the number of letters
- An accuracy factor that drops for each distinct mistake and each revealed word, down to a floor of 30%
- A speed bonus for finishing faster than expected, plus up to 50% extra for a combo of clean sentences in a row

Answers typed faster than a human can type lose the speed bonus and are flagged. All the numbers are settings that the admin can change, and the exact formula is in [the spec](SPEC.md#5-scoring-server-side).

## Coming back every day

- A daily goal of 10 sentences keeps the streak alive. Pro members get two automatic streak freezes a month
- Sentences with mistakes come back for review after 1, 3 and 7 days, and a clean review retires them
- Each week learners compete in groups of 30 inside seven tiers, from Bronze to Diamond. The top 7 move up and the bottom 5 move down
- A daily challenge gives everyone the same five sentences, with its own leaderboard and an optional summary posted to a channel
- Badges mark milestones such as the first lesson, streaks, perfect lessons and fast typing
- An optional bot reminder in the evening reaches learners who have a streak and have not met the day's goal

## The bot

`/start` sends a welcome message with a button that opens the Mini App and buttons to switch the language. The bot also takes Telegram Stars payments and sends reminders. It works through a webhook only.

Admins also get **Owner tools** in the bot: the number of lessons and of missing audio files, buttons that write the lessons and record the audio, and the switch between development mode and open to everyone. In development mode the bot answers admins only, so the owner can prepare everything before launch.

## Under the hood

One Rust binary, `eng`, serves the bot webhook, the JSON API and the Mini App, and runs the scheduled jobs (league rollover, daily challenge, reminders and cleanup).

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

The lessons are written once with Gemini, checked by automated rules and stored in PostgreSQL. Gemini also records the audio for every sentence and every word, and the files are served as static MP3s.

All durable data (users, progress, leagues, payments) lives in PostgreSQL. Short-lived data such as open attempts and unclaimed ad rewards is deleted within a day.

The reasons behind the main technical choices are in [DECISIONS.md](DECISIONS.md), and the full product behavior is in [SPEC.md](SPEC.md).

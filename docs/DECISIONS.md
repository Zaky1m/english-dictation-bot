# Decisions

Short records of why things are the way they are. Newest at the bottom.

## 1. Rust, axum and PostgreSQL

Context: the app is a small API plus a bot webhook, running on a shared server
with little RAM.
Decision: one Rust binary using axum and sqlx, with PostgreSQL for all durable
state.
Why: a single static-ish binary has a small memory footprint, and PostgreSQL
gives transactions for the money and reward paths.

## 2. Answers are checked on the server

Context: the learner types a sentence that the client could otherwise compare
locally.
Decision: the client sends typed letters; the server answers with wrong
positions only. The full sentence is sent after it is solved or a word is
revealed.
Why: the solution never reaches the client early, which keeps scoring and
rewards honest.

## 3. Text to speech is generated once and cached

Context: every user hears the same fixed sentence bank.
Decision: generate each sentence and word as an MP3 once, store it on disk and
serve it as a static file.
Why: no per-user TTS cost or latency, and MP3 plays in every Telegram WebView
including iOS.

## 4. Vanilla frontend

Context: the Mini App has a handful of screens and must load fast inside
Telegram.
Decision: plain HTML, CSS and JS in `web/`, with no build step and no
framework.
Why: nothing to compile or update, a tiny payload, and contributors can read it
directly.

## 5. No style prompt for text to speech

Context: the spec asks for a friendly teacher voice at a slightly slow pace.
Decision: send only the sentence text to the TTS model. Slow playback is the
client's 0.7x button.
Why: `gemini-3.8-flash-tts` rejects system instructions, and any style sentence
placed in the prompt is spoken aloud (a short prefix doubled the audio length).
The voice itself carries the character.

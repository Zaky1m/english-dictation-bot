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

## 6. Visual design choices

Context: Phase 1 presented palettes, layouts, fonts and voices as a design
canvas; the maintainer approved the result.
Decision: palette Pewter (background #B2B7BD, card #CBCFD4, accent #2F9BDD,
ink #11151A); Lexend for Latin text and Cairo for Arabic; voice Aoede; map as
a winding path with letter tiles for typing; the slow button is a labelled
"0.7x" pill next to the play button.
Why: chosen by the maintainer after comparing them on real screens. Pewter and
the ink color keep body text above 4.5:1 contrast on the gray background.

## 7. No account code

Context: the spec sketched a per-user account code for manual payment chats.
Decision: manual payments use the Telegram user id, so `users` has no
`account_code` column.
Why: the id is already unique and known to the bot, and the code added a
second identifier with no benefit.

## 8. Clean TTS output before encoding

Context: `gemini-3.8-flash-tts` puts a 30-60 ms pop about 20 ms in and a
130-150 ms full-scale burst after the speech, each separated from the speech by
170 ms or more of silence. They sound like a transmitter switching on and off.
Decision: `gemini::pcm::clean` drops a short isolated loud run at the start
(at most 80 ms, peak at least 15000) and at the end (at most 200 ms, peak at
least 30000) when 120 ms of silence separates it from the speech, keeping about
100 ms of lead and 200 ms of tail silence. `eng content audio --repair` applies
the same cleaning to existing files and backs up originals in `data/audio-orig/`.
Why: the artifacts come from the model, so the only fix is to remove them.
Amplitude is not compared with the speech because loud voices peak above the pop.

## 9. Gemini free tier limits TTS volume

Context: the free tier allows 10 requests per day per key for each TTS model.
Decision: generation resumes where it stopped (`eng content audio` only touches
files that are missing), so it can run over several days or on a paid key.
Why: 1200 sentences and about 1500 words need roughly 2700 calls.

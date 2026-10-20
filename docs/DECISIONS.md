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

## 10. API details the spec left open

- **Combo** is the number of consecutive clean sentences including the current
  one when it is clean, capped by `score_combo_cap`. A sentence with a mistake
  or reveal resets it to zero. It is stored in `users.combo` and updated in the
  solve transaction.
- **Reveal** is free for Pro users and for everyone while ads are disabled.
  With ads enabled, a free user needs an unclaimed ad credit, consumed
  atomically.
- **Review and challenge starts are POST** (`/api/review/next`,
  `/api/challenge/next`) because they create an attempt; `GET
  /api/challenge/today` only reports status.
- **Lesson unlock** follows one global order (situation position, then lesson
  number): a lesson opens when the one before it, even in the previous
  situation, is complete.
- **Daily limit** is enforced by one atomic upsert on `daily_usage`, serialized
  per user with an advisory lock, so double taps cannot overspend. Resuming an
  open attempt costs nothing.
- Streak, league points and badges are written inside the same solve
  transaction in Phase 7, where their rules and tests live.

## 11. Bot client and payments

- **Own thin Bot API client** (about 100 lines on reqwest) instead of a Telegram
  crate: the bot uses six methods, and the client can hide the token from error
  messages (reqwest errors include the URL, which contains the token).
- **Webhook registration** runs on every start and through `eng webhook set`.
  A failure is logged but does not stop the server. Do not start the server
  against the production bot token before the public URL is live, because it
  would point the bot at a dead address.
- **Webhook safety**: the path segment and the secret header are both compared
  in constant time. An update id is stored before handling and removed again if
  handling fails, so Telegram's retry is processed instead of skipped.
- **Stars**: the monthly plan is a recurring subscription (30 days, matching
  `subscription_period` 2592000); the yearly plan is a single payment of 365
  days. A purchase extends the current end date instead of overlapping it, and a
  payment is recorded once per `telegram_payment_charge_id` in one transaction
  together with the subscription row.
- **Menu button** text is `Eng`, because Telegram stores one text for all users.

## 12. Mini App structure and desktop use

- **No framework, no build**: ES modules under `web/` (`app.js` routes by URL
  hash; one file per screen). Screens read only the JSON API.
- **Exercise sources**: the same exercise screen serves lessons, reviews and the
  daily challenge through a small `source` object (how to get the next attempt
  and what to do when done), so there is one typing implementation.
- **Desktop**: content stays in a centered column (760 px) on a full-window
  background; Telegram Desktop and Web clients get `requestFullscreen`. Keyboard:
  Tab replays the sentence, Shift+Tab plays it slowly, Ctrl+Enter reveals the
  current word, Enter goes to the next sentence, Esc leaves, arrow keys move
  between words. Hints show only when a fine pointer is present.
- **Buttons never take focus from the letter boxes** (`pointerdown` is
  prevented), otherwise the phone keyboard closes whenever play is tapped.
- **Lesson result** is built from the running totals the client collects while
  the lesson is played; a reload during the result screen simply omits them.
- League, charts and badges screens come with their data in Phase 7.

## 13. Engagement rules

- **Daily goal and streak**: a day counts when the user finishes `streak_goal`
  lesson or review sentences that Cairo day (the daily challenge is separate).
  A Pro freeze covers each missed day in a gap, drawn from the monthly
  allowance, which refills at the first activity of a month. Free users restart
  at 1 after any missed day.
- **League points** are the scores of lesson and review sentences. A user joins a
  group of up to 30 in their tier on the first points of a week; a per-tier lock
  keeps groups from overfilling. The weekly job claims the week's `processed`
  flag first, so it can run twice without moving anyone twice. The week always
  starts on Saturday; the `league_week_start` setting is informational.
- **Scheduler**: one task wakes every 30 seconds and runs what is due. Reminders
  and the channel summary are switched off in the `settings` table
  (`reminders_enabled`, `challenge_post_enabled`) until the maintainer approves
  the wording. A user who blocked the bot is flagged `bot_blocked` (separate from
  the admin `blocked`) and cleared when they write to the bot again.
- **Fast typist** badge: a sentence of at least 20 letters typed at 150 ms per
  letter or faster without the anti-cheat flag.
- **Most missed** letters and words come from lifetime counters updated in the
  solve transaction, because attempts are purged after a day.

## 14. Ads and the admin panel

- **Ads need both switches**: the `ads_enabled` setting and `ADSGRAM_BLOCK_ID`.
  Until both are set, no ad is shown, the reward callback does nothing, and word
  reveals stay free for everyone.
- **Reward flow**: Adsgram calls `GET /api/adsgram/reward/{secret}?userid=...`,
  which adds one credit (at most three unclaimed at a time and one per 20
  seconds per user). The client retries its request for a few seconds because
  the callback arrives slightly after the ad ends.
- **Claiming**: `POST /api/ads/claim` accepts only `extra` (ten more sentences,
  once a day by setting). A word reveal has no separate claim: the reveal request
  itself spends the credit, atomically, so one credit can never pay twice.
- **Admin access** needs a valid `initData` and a user id in `ADMIN_IDS`; the
  page is served at `/admin` and every `/api/admin` route checks the same rule.
- **Settings editor** checks the JSON type of the new value against the old one
  and reloads the cache immediately.
- **New situations** are created hidden (`active = false`) with six empty
  lessons; a background task writes the sentences and audio, one task at a time
  (they share the Gemini quota), and the admin reviews the sentences before
  publishing. Task progress is stored in `content_jobs`; a restart marks
  interrupted tasks as failed, and re-running resumes only what is missing.
- **Revoking Pro** ends every subscription now, including future-dated
  extensions. A recurring Stars subscription must also be cancelled in Telegram.

## 15. Adding interface languages

Context: the project is public and translators should be able to add a language
without touching Rust or JavaScript.

Decision:
- `web/i18n/languages.json` lists the interface languages (code, native name,
  text direction). The server embeds the same file, so the bot and the Mini App
  never disagree. The first entry is the fallback for unknown codes.
- A build script embeds every `texts/<code>.toml`. Tests fail when a language
  is listed without its files or when any file misses or adds keys.
- The database no longer restricts `users.lang`; the API and the bot accept only
  listed codes.
- The bot shows one button per other language, labelled in that language.
- Small caps applies to English bot text only.
- Sentence translations, word meanings and situation names exist in Arabic (and
  situation names in English). Other interface languages show the English
  situation names. Translating the learning content itself is a separate,
  larger change.
- Translations arrive as pull requests, described in `TRANSLATING.md`. A hosted
  tool such as Weblate can come later if many translators join.

## 16. Development mode, owner tools and several ad networks

Context: people who run their own copy on a platform without a terminal still
need to write the lessons, and should be able to prepare everything before
anyone else sees the bot.

Decision:
- The `public_access` setting starts as false. Until an admin turns it on, the
  bot ignores everyone outside `ADMIN_IDS` (no reply, no user row) and the API
  refuses them with `not_public`, which sends the Mini App back to the bot chat.
- Admins get **Owner tools** in the bot: content counts, buttons that start the
  `lessons` and `audio` content tasks, and the public switch. Each button
  carries its action in its callback data, so the panel keeps no state.
- A restart records running content tasks as interrupted and starts them again,
  so a days-long audio run on the free Gemini tier continues by itself. Admins
  get a bot message when a task ends.
- Rewarded ads can come from Adsgram, Monetag, Adexium, Telega.io and GigaPub.
  The Mini App tries the configured networks in the order of the
  `ad_provider_order` setting and moves on when one has no ad.
- Only Adsgram confirms a watched ad server to server. The others confirm it
  inside the Mini App, which then reports it to `/api/ads/client-reward`. That
  report cannot be verified, so it gets a daily cap per learner
  (`client_ad_credits_per_day`) on top of the usual limits on open credits.
  Either way the reward is the same ad credit, spent atomically.
- PostgreSQL stays the only database. Every platform considered offers it, and a
  second database would double the data layer and weaken the transactional
  guarantees that payments rely on.

## 17. Audio from ElevenLabs or from files

Context: the free Gemini speech quota makes a full recording take days, and
some operators prefer another voice or their own recordings.

Decision:
- A `Speaker` picks ElevenLabs when both its key and voice id are set, and
  Gemini otherwise. Lessons are still written with Gemini.
- Every audio pass first adopts MP3 files already on disk at the expected paths,
  so hand-made or externally recorded files are never recorded again.
- The audio task no longer stops at the first quota wall. A pass that records
  nothing waits 30 minutes; the task gives up only after 72 idle passes.

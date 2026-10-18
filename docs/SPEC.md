# Product specification

Every decision here was agreed with the maintainer. Do not change behaviour
without asking. Numbers marked *(setting)* live in the `settings` table, are
editable from the admin panel, and apply immediately without a restart.

## 1. Audience and content

- General English for everyday situations and conversation, American accent.
- The UI language is Arabic (Modern Standard Arabic) by default, with an
  English option. The choice is stored per user and applies to both the Mini
  App and the bot messages.
- Content is a fixed sentence bank generated once with Gemini, reviewed by
  automated checks, and stored in the database:
  - 20 situations × 6 lessons × 10 sentences = 1200 sentences at launch.
  - Situation examples: greetings, café, restaurant, shopping, airport, hotel,
    directions, taxi and transport, doctor, pharmacy, phone calls, work and
    office, school and study, family and home, friends and plans, weather and
    small talk, bank and money, emergencies, online and tech, travel and
    sightseeing. Final list is decided during the content phase.
  - Difficulty rises inside each situation: lesson 1 has short, common
    sentences (3–6 words), lesson 6 has longer ones (10–16 words).
  - Each sentence stores: English text, Arabic translation, difficulty, token
    list, and for every word its meaning in context (Arabic).
  - New situations can be added later from the admin panel (generate → audio →
    review → publish).
- Audio is generated once per sentence and once per unique word, saved as MP3
  files, and reused for every user. MP3 is chosen for iOS WebView support.

## 2. Text to speech

- Model `gemini-3.8-flash-tts`; fallback `gemini-3.8-flash-lite-tts`.
- Female voice. Candidates to audition: `Erinome`, `Achernar`, `Aoede`,
  `Leda`, `Sulafat`. The maintainer picks one from samples during the design
  phase. Style instruction: a friendly English teacher, clear American
  pronunciation, natural pace leaning slightly slow.
- 25 API keys (`GEMINI_API_KEY_1` … `GEMINI_API_KEY_25`) are used round-robin.
  A key that returns 429 is put on cooldown (Retry-After or 60 s) and the next
  key is tried. A key that returns 400/403 for auth reasons is disabled for
  24 h and logged. When all keys are cooling down, the job waits for the
  earliest cooldown.
- The text model for content generation is `gemini-3.8-flash`, JSON output
  with a response schema, using the same key pool.

## 3. Map and progression

- Home screen is a map of situations; each situation has 6 lessons of 10
  sentences.
- A lesson unlocks when the previous one is complete. Pro users can open any
  lesson.
- Progress is saved per sentence; leaving mid-lesson resumes at the same place.

## 4. The exercise screen

- Shows the Arabic translation, a large play button, a slow button, and one
  input box per word.
- Input boxes:
  - The number of words and the length of each word are visible as a hint.
  - Only `a–z` / `A–Z` can be typed. Arabic letters, digits and everything else
    are ignored. Space or completing a word moves to the next box; Backspace in
    an empty box moves back.
  - Fixed characters (apostrophe, `?`, `.`, `,`, `!`, hyphen) are pre-printed
    in their position inside the box and are not typed.
  - Comparison is case-insensitive.
- Checking:
  - 500 ms after the last keystroke, the client sends the current letters to
    the server.
  - The server returns, per word, which letter positions are wrong. Wrong
    letters get a red underline.
  - The correct answer is never sent to the client until the sentence is
    solved or a word is revealed (only that word).
- Attempts are unlimited. The sentence completes automatically as soon as every
  word is correct; there is no submit button.
- Each mistake and each revealed word lowers the sentence score.
- Slow playback is the same file at 0.7× in the browser with pitch preserved.
- Reveal current word: Pro users reveal freely; free users watch a rewarded ad
  (if ads are enabled).
- After solving: every word is tappable. Tapping plays the word audio and shows
  its meaning, with a button to save it to "My words".

## 5. Scoring (server side)

All coefficients are settings.

```
letters      = number of letters to type in the sentence
base         = 10 + letters
penalty      = 0.10 per distinct mistake + 0.25 per revealed word
accuracy     = max(0.3, 1 - penalty)
expected_ms  = 4000 + 450 * letters
speed_bonus  = time_ms < expected_ms ? round(base * 0.5 * (1 - time_ms / expected_ms)) : 0
combo        = consecutive sentences with zero mistakes and zero reveals (cap 5)
score        = round((base * accuracy + speed_bonus) * (1 + 0.1 * combo))
```

- Time starts when the audio is first played for that attempt.
- A distinct mistake is a (word, position, wrong letter) seen for the first time
  in the attempt.
- Anti-cheat: if `time_ms < 80 * letters` the speed bonus is zero and the
  attempt is flagged. Checks faster than one per 250 ms per attempt are
  rejected (HTTP 429).

## 6. Leagues

- Weekly leagues. The week starts Saturday 00:00 Africa/Cairo *(setting)*.
- Tiers: Bronze, Silver, Gold, Sapphire, Ruby, Emerald, Diamond.
- A user joins a group of up to 30 in their tier when they earn their first
  points of the week.
- At week end: top 7 promote, bottom 5 demote *(settings)*; nobody leaves the
  edges (no demotion from Bronze, no promotion from Diamond). The job is
  idempotent.
- Pro users show a Pro badge and a distinct name color in the league.

## 7. Daily limits, Pro, payments

- Free: 10 sentences per day *(setting)*. One rewarded ad per day *(setting)*
  unlocks 10 more sentences *(setting)*.
- Pro: 50 sentences per day *(setting)*, reveal without ads, detailed
  statistics, Pro badge and name color in leagues, open any lesson, streak
  freeze (2 per month *(setting)*).
- The daily counter counts sentences started that day (Cairo date), including
  reviews. The daily challenge does not count.
- Prices: month 50 EGP or 2 USD; year 500 EGP or 20 USD.
- Telegram Stars, automatic:
  - Month: 150 Stars as a recurring subscription (`subscription_period`
    2592000).
  - Year: 1500 Stars, one-time.
  - Both prices are settings.
- Vodafone Cash, InstaPay, USDT: manual. A button opens a chat with the admin
  username with a prefilled message containing the user's account code. The
  admin grants Pro from the admin panel. Admin username and IDs come from env
  and may be empty at first; the button hides while empty.

## 8. Ads (Adsgram)

- Global on/off switch *(setting)*, block ID from env, off until configured.
- Placements: reveal the current word; unlock extra sentences after the daily
  limit.
- Reward flow: Adsgram calls `GET /api/adsgram/reward/{secret}?userid=…`. The
  server adds one ad credit for that user (rate-limited). The client then
  claims a specific reward, which consumes the credit atomically. Unclaimed
  credits expire after 10 minutes.

## 9. Retention features (all in v1)

- Daily streak: a day counts when the user completes 10 sentences
  *(setting)* that Cairo day. Pro streak freeze covers a missed day
  automatically.
- Daily reminder: one bot message per day at 20:00 Cairo *(setting)* to users
  with a streak who have not met today's goal; a button turns reminders off.
  The message wording is approved by the maintainer before launch.
- Mistake review (spaced repetition): sentences with mistakes or reveals return
  after 1, 3, then 7 days. A clean review graduates the sentence.
- Daily challenge: the same 5 sentences for everyone each day, with its own
  daily leaderboard. A daily summary (participants, top 10, hardest word) is
  posted to a channel (`CHALLENGE_CHANNEL_ID`). The visual style of that post
  is approved by the maintainer.
- Badges: first lesson; 10/100/500/1000 sentences; streak 3/7/30/100; perfect
  lesson; fast typist; first promotion; situation completed.
- Progress stats: free users see a summary (sentences, words learned, streak,
  accuracy). Pro users see charts over time, most-missed letters and words.
- Postponed (not in v1): share card, referral rewards.

## 10. Design

- Background: medium gray, neither light nor dark. Accent: sky blue.
- No waves, no glows, no glowing buttons, no emoji. Icons are SVG.
- Important elements are centered.
- Fonts: Cairo for Arabic. A clean Latin font for English (chosen from options
  in the design phase). RTL for Arabic UI, LTR for English UI; the sentence
  area is always LTR.
- Bot messages: Arabic by default, English when the user switches. English bot
  text is rendered in small caps (see §12). The Mini App uses normal letters.
- Every screen is approved by the maintainer as a visual design before it is
  coded.

## 11. Admin panel

- Opened as a Mini App page (`/admin`) from a bot button shown only to admins.
  Access requires valid `initData` and a user ID in `ADMIN_IDS`.
- Sections: overview stats; user search and Pro grant/revoke (month, year,
  custom days); payments list; settings editor; content (situations, generate a
  new situation, regenerate audio, disable a sentence).

## 12. Bot

- `/start`: welcome message in the user's language with a button that opens the
  Mini App. Language switch button.
- Handles `pre_checkout_query` and `successful_payment` for Stars.
- Message style: title line starts with `⌯`, other lines start with `•`,
  label/value separated by `↢` in Arabic and `↣` in English. No emoji, minimal
  punctuation, colored buttons via the `style` field
  (`success`/`primary`/`danger`).
- Small caps for English bot text: map `a–z` and `A–Z` to
  `ᴀʙᴄᴅᴇꜰɢʜɪᴊᴋʟᴍɴᴏᴘǫʀѕᴛᴜᴠᴡxʏᴢ`, except inside `<code>…</code>`, `{placeholders}`,
  HTML tags, `/commands`, `@usernames`, links, and names. Applied at render
  time; source strings stay plain English.

## 13. State lifetime

Temporary (deleted after at most 24 hours):
- An open sentence attempt (typed letters, start time).
- Unclaimed ad credits (10 minutes).
- Unpaid Stars invoices.
- Processed `update_id` records used for webhook de-duplication.

Durable (kept):
- Users, language, settings per user, progress, scores, streaks and freezes.
- Leagues, groups and weekly history.
- Subscriptions and payments.
- Review queue, saved words, badges, statistics.
- Global settings, sentence bank, and audio files.

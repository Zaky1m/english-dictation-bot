# Deployment

[العربية](deploy.ar.md) · [Back to the README](../README.md)

## Before you start

You need:

- A Telegram bot token from [BotFather](https://t.me/BotFather)
- Gemini API keys from [Google AI Studio](https://aistudio.google.com/apikey) to generate the lessons and their audio
- Three random secrets for `WEBHOOK_PATH`, `WEBHOOK_SECRET` and `ADSGRAM_REWARD_SECRET`

```bash
openssl rand -hex 24              # run once per secret
```

### The address

Telegram opens Mini Apps and sends webhooks over HTTPS only, so the app needs a public HTTPS address. That address goes into `PUBLIC_URL`, with no trailing slash. There are two ways to get one:

- The free address of the hosting platform, such as `https://your-app.up.railway.app`. It works as it is, with HTTPS included
- Your own domain or subdomain, such as `https://learn.example.com`. Point it at the server or the platform from your DNS provider, as described for each option below

On every start the app registers the webhook and the bot's menu button with that address, so changing the domain only needs a new `PUBLIC_URL` and a restart.

### Lessons and audio

The repository ships without lessons. After the first start, generate them once from inside the running app:

```bash
eng content generate --all        # sentences, translations and word meanings
eng content audio                 # audio for every sentence and every word
```

Both commands resume where they stopped, so they can be run again after an interruption. The free Gemini tier allows a limited number of speech requests per key per day, so audio can take several days with few keys. More keys, or one key with billing enabled, shorten that.

## Environment variables

`.env.example` lists every variable with a short comment.

| Variable | Purpose |
|---|---|
| `BOT_TOKEN` | Telegram bot token |
| `PUBLIC_URL` | Public HTTPS address of the Mini App |
| `DATABASE_URL` | PostgreSQL connection string |
| `BIND_ADDR` | Address to listen on. When empty, the app uses `0.0.0.0:$PORT` |
| `WEBHOOK_PATH`, `WEBHOOK_SECRET` | Webhook path segment and secret header |
| `ADSGRAM_REWARD_SECRET`, `ADSGRAM_BLOCK_ID` | Rewarded ads, off while the block id is empty |
| `ADMIN_IDS`, `ADMIN_USERNAME` | Admin Telegram ids, and the contact for manual payments |
| `CHALLENGE_CHANNEL_ID` | Channel that receives the daily challenge summary |
| `DATA_DIR`, `WEB_DIR` | Audio directory and Mini App directory |
| `GEMINI_API_KEY_<n>` | Gemini keys, used in turn |

Limits, prices and scoring rules are not environment variables. They live in the database and change from the admin panel without a restart.

## Option 1: a VPS with Docker

Any Linux server with Docker works.

1. Point your domain at the server: an `A` record for `learn.example.com` with the server's IP address
2. Get the code and fill in the settings

```bash
git clone https://github.com/<you>/english-dictation-bot.git
cd english-dictation-bot
cp .env.example .env
nano .env                         # BOT_TOKEN, PUBLIC_URL, secrets, Gemini keys, POSTGRES_PASSWORD
```

3. Start the app and its database

```bash
docker compose up -d --build      # the app listens on 127.0.0.1:4510
```

4. Put HTTPS in front of it. [Caddy](https://caddyserver.com/docs/install) gets and renews the certificate by itself. Add this to `/etc/caddy/Caddyfile` and reload Caddy:

```
learn.example.com {
    reverse_proxy 127.0.0.1:4510
}
```

nginx with certbot works the same way, proxying the domain to `127.0.0.1:4510`.

5. Generate the lessons

```bash
docker compose run --rm app eng content generate --all
docker compose run --rm app eng content audio
```

Updating later is a pull and a rebuild:

```bash
git pull && docker compose up -d --build
```

## Option 2: Railway

1. Fork the repository on GitHub
2. In [Railway](https://railway.com), create a project from the forked repository. Railway builds it with the included `Dockerfile`
3. Add a PostgreSQL database to the project, then set `DATABASE_URL` on the app service to `${{Postgres.DATABASE_URL}}`
4. Add the other variables from `.env.example` to the app service. Leave `BIND_ADDR` out, because the app listens on the `PORT` that Railway provides
5. Attach a volume to the app service at `/app/data`, where the audio files live. The image runs as a non-root user, so also set `RAILWAY_RUN_UID=0` to let it write to the volume
6. Under Settings, then Networking, generate a domain or add your own. For your own domain, create the `CNAME` record that Railway shows at your DNS provider
7. Set `PUBLIC_URL` to that address and redeploy
8. Generate the lessons with the [Railway CLI](https://docs.railway.com/guides/cli):

```bash
railway ssh -- eng content generate --all
railway ssh --session             # then run: eng content audio
```

The `--session` option keeps the long audio job running if the connection drops.

## Other platforms

The same image runs on any platform that builds a Dockerfile and offers PostgreSQL and persistent storage. The pattern is always the same:

| Step | What to do |
|---|---|
| Build | From the repository's `Dockerfile` |
| Database | A managed PostgreSQL 16, with its URL in `DATABASE_URL` |
| Storage | A persistent disk or volume mounted at `/app/data` |
| Port | Leave `BIND_ADDR` unset. The app uses the platform's `PORT`, or 4510 by default |
| Address | The platform's HTTPS address or your own domain, written into `PUBLIC_URL` |
| Lessons | Run the two `eng content` commands from the platform's shell once |

On [Render](https://render.com) that is a Docker web service with a persistent disk and the Shell tab. On [Fly.io](https://fly.io) it is `fly launch`, a volume from `fly volumes create`, and `fly ssh console` for the commands.

## After the first start

- Open the bot in Telegram and send `/start`. The menu button and the welcome button open the Mini App
- Put your Telegram user id in `ADMIN_IDS` to see the admin panel button
- Set `ADMIN_USERNAME` to offer manual payments, `CHALLENGE_CHANNEL_ID` to post the daily challenge summary, and `ADSGRAM_BLOCK_ID` for rewarded ads
- Back up the PostgreSQL database. The audio can be generated again, but users and payments cannot

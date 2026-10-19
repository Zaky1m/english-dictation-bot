# Deployment

[العربية](deploy.ar.md) · [Back to the README](../README.md)

## Before you start

You need:

- A Telegram bot token from [BotFather](https://t.me/BotFather)
- Gemini API keys from [Google AI Studio](https://aistudio.google.com/apikey) to generate the lessons and their audio
- Three random secrets for `WEBHOOK_PATH`, `WEBHOOK_SECRET` and `ADSGRAM_REWARD_SECRET`
- Your own Telegram user id for `ADMIN_IDS`. Any user-info bot, such as [@userinfobot](https://t.me/userinfobot), shows it

```bash
openssl rand -hex 24              # run once per secret
```

### The address

Telegram opens Mini Apps and sends webhooks over HTTPS only, so the app needs a public HTTPS address. That address goes into `PUBLIC_URL`, with no trailing slash. There are two ways to get one:

- The free address of the hosting platform, such as `https://your-app.up.railway.app`. It works as it is, with HTTPS included
- Your own domain or subdomain, such as `https://learn.example.com`. Point it at the server or the platform from your DNS provider, as described for each option below

On every start the app registers the webhook and the bot's menu button with that address, so changing the domain only needs a new `PUBLIC_URL` and a restart.

### Development mode

A new install starts in development mode. Only the admins listed in `ADMIN_IDS` get answers: the bot ignores everyone else, and the Mini App sends them straight back to the bot chat. This leaves time to write the lessons and try everything before anyone else sees it. Set `ADMIN_IDS` before the first start, or nobody can use the bot.

### Lessons and audio

The repository ships without lessons, and the owner writes them from the bot, with no terminal needed:

1. Send `/start` to the bot and tap **Owner tools**
2. Tap **Write lessons**. The app writes every situation with Gemini and then records the audio
3. A message arrives when the task ends. **Refresh** shows the counts at any time, and **Record audio** fills in any audio still missing

A task carries on by itself after a restart. The free Gemini tier allows a limited number of speech requests per key per day, so the audio can take several days with few keys. More keys, or one key with billing enabled, shorten that.

When everything looks right, tap **Open to everyone** in the same panel. **Back to development** closes it again.

On a server with a terminal the same work can run from the command line:

```bash
eng content generate --all        # sentences, translations and word meanings
eng content audio                 # audio for every sentence and every word
```

## Environment variables

`.env.example` lists every variable with a short comment.

| Variable | Purpose |
|---|---|
| `BOT_TOKEN` | Telegram bot token |
| `PUBLIC_URL` | Public HTTPS address of the Mini App |
| `DATABASE_URL` | PostgreSQL connection string |
| `BIND_ADDR` | Address to listen on. When empty, the app uses `0.0.0.0:$PORT` |
| `WEBHOOK_PATH`, `WEBHOOK_SECRET` | Webhook path segment and secret header |
| `ADMIN_IDS` | Telegram ids of the admins, who are also the only users in development mode |
| `ADMIN_USERNAME` | Contact shown for manual payments |
| `ADSGRAM_BLOCK_ID`, `ADSGRAM_REWARD_SECRET` | Adsgram rewarded ads and its reward callback |
| `MONETAG_ZONE_ID` | Monetag rewarded ads |
| `ADEXIUM_WIDGET_ID` | Adexium rewarded ads |
| `TELEGA_TOKEN`, `TELEGA_AD_BLOCK_UUID` | Telega.io rewarded ads |
| `GIGAPUB_PROJECT_ID` | GigaPub rewarded ads |
| `CHALLENGE_CHANNEL_ID` | Channel that receives the daily challenge summary |
| `DATA_DIR`, `WEB_DIR` | Audio directory and Mini App directory |
| `GEMINI_API_KEY_<n>` | Gemini keys, used in turn |

Every ad network is optional, and any mix of them works. Limits, prices, scoring rules and the order in which ad networks are tried are not environment variables. They live in the database and change from the admin panel without a restart.

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

5. Write the lessons from **Owner tools** in the bot, or from the terminal:

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
4. Add the other variables from `.env.example` to the app service, including your id in `ADMIN_IDS`. Leave `BIND_ADDR` out, because the app listens on the `PORT` that Railway provides
5. Attach a volume to the app service at `/app/data`, where the audio files live. The image runs as a non-root user, so also set `RAILWAY_RUN_UID=0` to let it write to the volume
6. Under Settings, then Networking, generate a domain or add your own. For your own domain, create the `CNAME` record that Railway shows at your DNS provider
7. Set `PUBLIC_URL` to that address and redeploy
8. Write the lessons from **Owner tools** in the bot, as described in [Lessons and audio](#lessons-and-audio)

## Other platforms

The same image runs on any platform that builds a Dockerfile and offers PostgreSQL and persistent storage. The pattern is always the same:

| Step | What to do |
|---|---|
| Build | From the repository's `Dockerfile` |
| Database | A managed PostgreSQL 16, with its URL in `DATABASE_URL` |
| Storage | A persistent disk or volume mounted at `/app/data` |
| Port | Leave `BIND_ADDR` unset. The app uses the platform's `PORT`, or 4510 by default |
| Address | The platform's HTTPS address or your own domain, written into `PUBLIC_URL` |
| Lessons | From **Owner tools** in the bot |

On [Render](https://render.com) that is a Docker web service with a persistent disk. On [Fly.io](https://fly.io) it is `fly launch` with a volume from `fly volumes create`.

## After the first start

- Open the bot in Telegram and send `/start`. The menu button and the welcome button open the Mini App, and admins also see the admin panel and **Owner tools**
- Write the lessons, try the app, then open it to everyone from **Owner tools**
- Set `ADMIN_USERNAME` to offer manual payments, `CHALLENGE_CHANNEL_ID` to post the daily challenge summary, and the ids of any ad networks you use
- Back up the PostgreSQL database. The audio can be generated again, but users and payments cannot

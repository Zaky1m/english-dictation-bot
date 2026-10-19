# Translating

The app interface and the bot messages can be translated into any language. Each language is two text files plus one line in a list, and no programming is needed.

## What you translate

| File | What it holds |
|---|---|
| `web/i18n/<code>.json` | Every label and message in the Mini App |
| `texts/<code>.toml` | The messages the Telegram bot sends |
| `web/i18n/languages.json` | The list of languages, with the name shown in the language picker |

`<code>` is the language's [ISO 639-1 code](https://en.wikipedia.org/wiki/List_of_ISO_639_language_codes), for example `fr`, `tr` or `ur`.

## Fix a translation

Edit the line in the language's two files and open a pull request. Small fixes are welcome, even a single word.

## Add a language

1. Copy `web/i18n/en.json` to `web/i18n/<code>.json` and translate every value
2. Copy `texts/en.toml` to `texts/<code>.toml` and translate every value
3. Add your language to `web/i18n/languages.json`, with its name written in the language itself and `"dir": "rtl"` for right-to-left scripts

```json
{ "code": "fr", "name": "Français", "dir": "ltr" }
```

4. Open a pull request. The checks fail if a key is missing or misspelled, and the failure names the key

To see it before sending, run the app (see the [deployment guide](docs/deploy.md)) and pick your language in Settings.

## Rules for the text

- Translate the values only. Keys on the left stay in English and unchanged
- Keep placeholders such as `{name}`, `{n}` and `{total}` exactly as written. The app fills them in
- Keep HTML tags such as `<code>` and the `/start` command as they are
- Bot messages start with `⌯` and list their lines with `•`. Keep both
- The arrow before a value follows the reading direction: `↣` for left-to-right languages and `↢` for right-to-left ones
- No emoji. The interface uses icons instead
- Write normal letters. English bot text is shown in small caps by the app itself, so the files stay plain

## What is not translatable yet

The learning content (sentence translations and word meanings) exists in Arabic only, and the lesson names in Arabic and English. A learner who picks another interface language still sees Arabic translations under each sentence. Making the content itself multilingual is a larger change, and a proposal in an issue is the place to start.

## No pull request experience?

Open an issue with the **Translation** template and paste your text there. A maintainer will add it for you.

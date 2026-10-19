# Pro membership

[العربية](pro.ar.md) · [Back to the README](../README.md)

## Free and Pro

| | Free | Pro |
|---|---|---|
| Sentences per day | 10, plus 10 more after one rewarded ad | 50 |
| Reveal a word | After a rewarded ad | Any time |
| Lessons | In order, each after the previous one | Any lesson |
| Streak freeze | None | 2 a month, used automatically |
| Statistics | Summary: sentences, words, streak, accuracy | Charts over time, most-missed letters and words |
| League | Standard name | Pro badge and a distinct name color |

The daily count includes reviews and resets at midnight Cairo time. The daily challenge does not count. Every number in this table is a setting the admin can change.

## Paying

### Telegram Stars

Stars payments are automatic. The learner pays inside Telegram and Pro starts as soon as the payment arrives.

- Monthly: 150 Stars as a recurring subscription that Telegram renews every 30 days
- Yearly: 1,500 Stars, paid once for 365 days

A recurring monthly plan is cancelled from the learner's Telegram settings.

### Manual payment

Local payment methods, such as mobile wallets, bank transfers or USDT, work through the admin. The learner taps a method and the app opens a chat with the admin, with a ready message that includes the learner's account id. After receiving the money the admin grants Pro from the admin panel for a month, a year or a custom number of days.

The manual option appears only when `ADMIN_USERNAME` is set. The defaults show prices of 50 EGP or 2 USD a month and 500 EGP or 20 USD a year.

## For operators

- Prices, limits and the number of streak freezes are in the `settings` table and change from the admin panel without a restart
- Every payment is recorded once by its Telegram charge id, so a repeated update never grants Pro twice
- Revoking Pro in the admin panel ends every active period at once, including extensions that start in the future
- Rewarded ads can come from Adsgram, Monetag, Adexium, Telega.io and GigaPub. Set the ids of the networks you use and turn on the `ads_enabled` setting. The app tries the networks in the order of the `ad_provider_order` setting until one has an ad. Without any network, reveals stay free for everyone
- Adsgram confirms a watched ad to the server directly. The other networks confirm it only inside the app, so their rewards are capped per learner per day by the `client_ad_credits_per_day` setting

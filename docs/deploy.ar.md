# التشغيل على سيرفر

[English](deploy.md) · [العودة إلى README](../README.ar.md)

## قبل البداية

تحتاج إلى:

- توكن بوت تيليجرام من [BotFather](https://t.me/BotFather)
- مفاتيح Gemini من [Google AI Studio](https://aistudio.google.com/apikey) لتوليد الدروس وصوتها
- ثلاثة أسرار عشوائية لـ`WEBHOOK_PATH` و`WEBHOOK_SECRET` و`ADSGRAM_REWARD_SECRET`

```bash
openssl rand -hex 24              # run once per secret
```

### العنوان

تيليجرام يفتح الميني آب ويرسل الويبهوك عبر HTTPS فقط، لذلك يحتاج التطبيق إلى عنوان HTTPS عام. هذا العنوان يوضع في `PUBLIC_URL` بدون شرطة مائلة في آخره. هناك طريقتان للحصول عليه:

- العنوان المجاني من منصة الاستضافة، مثل `https://your-app.up.railway.app`، ويعمل كما هو ومعه HTTPS
- دومين أو ساب دومين خاص بك، مثل `https://learn.example.com`، توجّهه إلى السيرفر أو المنصة من مزوّد الـDNS كما هو موضح في كل طريقة بالأسفل

في كل تشغيل يسجّل التطبيق الويبهوك وزر القائمة في البوت بهذا العنوان، فتغيير الدومين يحتاج فقط إلى `PUBLIC_URL` جديد وإعادة تشغيل.

### الدروس والصوت

المستودع لا يحتوي على الدروس. بعد أول تشغيل ولّدها مرة واحدة من داخل التطبيق العامل:

```bash
eng content generate --all        # sentences, translations and word meanings
eng content audio                 # audio for every sentence and every word
```

الأمران يكملان من حيث توقفا، فيمكن تشغيلهما مرة أخرى بعد أي انقطاع. الخطة المجانية في Gemini تسمح بعدد محدود من طلبات الصوت لكل مفتاح في اليوم، فقد يستغرق الصوت عدة أيام مع مفاتيح قليلة. مفاتيح أكثر، أو مفتاح واحد مفعّل عليه الدفع، يختصر ذلك.

## متغيرات البيئة

الملف `.env.example` يسرد كل المتغيرات مع تعليق قصير لكل منها.

| المتغير | الغرض |
|---|---|
| `BOT_TOKEN` | توكن بوت تيليجرام |
| `PUBLIC_URL` | عنوان HTTPS العام للميني آب |
| `DATABASE_URL` | رابط الاتصال بـPostgreSQL |
| `BIND_ADDR` | العنوان الذي يستمع عليه التطبيق. إذا كان فارغًا يستخدم `0.0.0.0:$PORT` |
| `WEBHOOK_PATH` و`WEBHOOK_SECRET` | جزء مسار الويبهوك والهيدر السري |
| `ADSGRAM_REWARD_SECRET` و`ADSGRAM_BLOCK_ID` | إعلانات المكافأة، وتتوقف ما دام الـblock id فارغًا |
| `ADMIN_IDS` و`ADMIN_USERNAME` | أرقام الأدمنز على تيليجرام، وحساب التواصل للدفع اليدوي |
| `CHALLENGE_CHANNEL_ID` | القناة التي يُنشر فيها ملخص التحدي اليومي |
| `DATA_DIR` و`WEB_DIR` | مجلد الصوت ومجلد الميني آب |
| `GEMINI_API_KEY_<n>` | مفاتيح Gemini، تُستخدم بالتناوب |

الحدود والأسعار وقواعد النقاط ليست متغيرات بيئة. هي محفوظة في قاعدة البيانات وتتغير من لوحة الأدمن دون إعادة تشغيل.

## الطريقة الأولى: VPS مع Docker

أي سيرفر لينكس عليه Docker يكفي.

1. وجّه الدومين إلى السيرفر بسجل `A` لـ`learn.example.com` بعنوان IP الخاص بالسيرفر
2. نزّل الكود واملأ الإعدادات

```bash
git clone https://github.com/<you>/english-dictation-bot.git
cd english-dictation-bot
cp .env.example .env
nano .env                         # BOT_TOKEN, PUBLIC_URL, secrets, Gemini keys, POSTGRES_PASSWORD
```

3. شغّل التطبيق وقاعدة البيانات

```bash
docker compose up -d --build      # the app listens on 127.0.0.1:4510
```

4. ضع HTTPS أمامه. [Caddy](https://caddyserver.com/docs/install) يستخرج الشهادة ويجددها وحده. أضف هذا إلى `/etc/caddy/Caddyfile` ثم أعد تحميل Caddy:

```
learn.example.com {
    reverse_proxy 127.0.0.1:4510
}
```

ويعمل nginx مع certbot بنفس الطريقة، بتوجيه الدومين إلى `127.0.0.1:4510`.

5. ولّد الدروس

```bash
docker compose run --rm app eng content generate --all
docker compose run --rm app eng content audio
```

التحديث بعد ذلك سحب للكود وإعادة بناء:

```bash
git pull && docker compose up -d --build
```

## الطريقة الثانية: Railway

1. اعمل fork للمستودع على GitHub
2. في [Railway](https://railway.com) أنشئ مشروعًا من المستودع الذي عملت له fork، وسيبنيه Railway بالـ`Dockerfile` الموجود
3. أضف قاعدة PostgreSQL إلى المشروع، ثم اضبط `DATABASE_URL` في خدمة التطبيق على `${{Postgres.DATABASE_URL}}`
4. أضف باقي المتغيرات من `.env.example` إلى خدمة التطبيق. لا تضف `BIND_ADDR`، لأن التطبيق يستمع على `PORT` الذي يوفّره Railway
5. اربط volume بخدمة التطبيق على المسار `/app/data` حيث ملفات الصوت. الصورة تعمل بمستخدم غير root، فاضبط أيضًا `RAILWAY_RUN_UID=0` ليتمكن من الكتابة على الـvolume
6. من Settings ثم Networking ولّد دومين أو أضف دومينك. لدومينك الخاص أنشئ سجل `CNAME` الذي يعرضه Railway عند مزوّد الـDNS
7. اضبط `PUBLIC_URL` على هذا العنوان وأعد النشر
8. ولّد الدروس بـ[Railway CLI](https://docs.railway.com/guides/cli):

```bash
railway ssh -- eng content generate --all
railway ssh --session             # then run: eng content audio
```

خيار `--session` يُبقي مهمة الصوت الطويلة شغالة إذا انقطع الاتصال.

## منصات أخرى

نفس الصورة تعمل على أي منصة تبني Dockerfile وتوفر PostgreSQL وتخزينًا دائمًا. الخطوات دائمًا واحدة:

| الخطوة | ما تفعله |
|---|---|
| البناء | من الـ`Dockerfile` الموجود في المستودع |
| قاعدة البيانات | PostgreSQL 16 مُدارة، ورابطها في `DATABASE_URL` |
| التخزين | قرص أو volume دائم على المسار `/app/data` |
| البورت | اترك `BIND_ADDR` فارغًا، فيستخدم التطبيق `PORT` من المنصة أو 4510 افتراضيًا |
| العنوان | عنوان HTTPS من المنصة أو دومينك الخاص، في `PUBLIC_URL` |
| الدروس | شغّل أمري `eng content` مرة واحدة من shell المنصة |

على [Render](https://render.com) هذا يعني Docker web service مع persistent disk وتبويب Shell. وعلى [Fly.io](https://fly.io) يعني `fly launch` وvolume من `fly volumes create` و`fly ssh console` لتشغيل الأوامر.

## بعد أول تشغيل

- افتح البوت في تيليجرام وأرسل `/start`. زر القائمة وزر الترحيب يفتحان الميني آب
- ضع رقم حسابك على تيليجرام في `ADMIN_IDS` ليظهر زر لوحة الأدمن
- اضبط `ADMIN_USERNAME` لتفعيل الدفع اليدوي، و`CHALLENGE_CHANNEL_ID` لنشر ملخص التحدي اليومي، و`ADSGRAM_BLOCK_ID` لإعلانات المكافأة
- خذ نسخة احتياطية من قاعدة PostgreSQL. الصوت يمكن توليده من جديد، لكن المستخدمين والمدفوعات لا

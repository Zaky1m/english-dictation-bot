# eng

ميني آب تيليجرام لتعلّم الإنجليزي بالإملاء: المتعلّم يسمع الجملة ويشوف ترجمتها
بالعربي ويكتبها بالإنجليزي. الباك إند Rust (axum + PostgreSQL).

## الحالة

المرحلة 0 (الهيكل) والمرحلة 1 (التصميم) والمرحلة 2 (قاعدة البيانات) خلصوا. `eng serve` بيتصل بـPostgreSQL ويطبّق الـmigrations عند التشغيل، وفيه `GET /health`، وأمر `eng content voices` لعينات الأصوات.

## التشغيل

```bash
cp .env.example .env   # واملا القيم
cargo run --release -- serve
```

## المتطلبات

PostgreSQL 16 وffmpeg (لتوليد الصوت لاحقًا). المتغيرات كلها في `.env.example`.

//! End-to-end API flow against a throwaway schema. Skipped unless
//! `TEST_DATABASE_URL` is set.
use super::*;
use crate::db;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use hmac::{Hmac, KeyInit, Mac};
use serde_json::{Value, json};
use sha2::Sha256;
use tower::ServiceExt;

const TOKEN: &str = "123456:TEST-TOKEN";

pub(super) fn init_data(user_id: i64) -> String {
    let user = format!(r#"{{"id":{user_id},"first_name":"T"}}"#);
    let auth_date = chrono::Utc::now().timestamp().to_string();
    let mut fields = [("auth_date", auth_date.as_str()), ("user", user.as_str())];
    fields.sort();
    let dcs = fields
        .iter()
        .map(|(k, v)| format!("{k}={v}"))
        .collect::<Vec<_>>()
        .join("\n");
    let mut key = Hmac::<Sha256>::new_from_slice(b"WebAppData").unwrap();
    key.update(TOKEN.as_bytes());
    let mut mac = Hmac::<Sha256>::new_from_slice(&key.finalize().into_bytes()).unwrap();
    mac.update(dcs.as_bytes());
    let hash = hex::encode(mac.finalize().into_bytes());
    form_urlencoded::Serializer::new(String::new())
        .append_pair("auth_date", &auth_date)
        .append_pair("user", &user)
        .append_pair("hash", &hash)
        .finish()
}

struct Client {
    app: Router,
    auth: String,
}

impl Client {
    async fn call(&self, method: &str, path: &str, body: Option<Value>) -> (StatusCode, Value) {
        let mut req = Request::builder()
            .method(method)
            .uri(path)
            .header("authorization", format!("tma {}", self.auth));
        let body = match body {
            Some(b) => {
                req = req.header("content-type", "application/json");
                Body::from(b.to_string())
            }
            None => Body::empty(),
        };
        let res = self
            .app
            .clone()
            .oneshot(req.body(body).unwrap())
            .await
            .unwrap();
        let status = res.status();
        let bytes = axum::body::to_bytes(res.into_body(), 1 << 20)
            .await
            .unwrap();
        (
            status,
            serde_json::from_slice(&bytes).unwrap_or(Value::Null),
        )
    }
}

async fn seed(pool: &PgPool) -> (i32, i32) {
    sqlx::query("INSERT INTO situations (slug, name_ar, name_en, icon, position) VALUES ('t', 'ت', 'T', 'chat', 0)")
        .execute(pool)
        .await
        .unwrap();
    let sid: i32 = sqlx::query_scalar("SELECT id FROM situations")
        .fetch_one(pool)
        .await
        .unwrap();
    let mut lessons = Vec::new();
    for number in 1..=2 {
        let id: i32 = sqlx::query_scalar(
            "INSERT INTO lessons (situation_id, number) VALUES ($1, $2) RETURNING id",
        )
        .bind(sid)
        .bind(number)
        .fetch_one(pool)
        .await
        .unwrap();
        lessons.push(id);
    }
    let sentences = [
        (lessons[0], 1, "I am fine."),
        (lessons[0], 2, "Good morning."),
        (lessons[1], 1, "Thank you."),
    ];
    for (lesson, position, text) in sentences {
        let tokens = crate::exercise::tokens::tokenize(text).unwrap();
        let id: i32 = sqlx::query_scalar(
            "INSERT INTO sentences (lesson_id, position, text, normalized, translation, difficulty, tokens)
             VALUES ($1, $2, $3, $4, 'ترجمة', 1, $5) RETURNING id",
        )
        .bind(lesson)
        .bind(position as i16)
        .bind(text)
        .bind(crate::exercise::tokens::normalize(text))
        .bind(serde_json::to_value(&tokens).unwrap())
        .fetch_one(pool)
        .await
        .unwrap();
        for (idx, t) in tokens.iter().enumerate() {
            sqlx::query("INSERT INTO words (word) VALUES ($1) ON CONFLICT DO NOTHING")
                .bind(&t.word)
                .execute(pool)
                .await
                .unwrap();
            sqlx::query("INSERT INTO sentence_words (sentence_id, idx, word, meaning_ar) VALUES ($1, $2, $3, 'معنى')")
                .bind(id)
                .bind(idx as i16)
                .bind(&t.word)
                .execute(pool)
                .await
                .unwrap();
        }
    }
    (lessons[0], lessons[1])
}

#[tokio::test]
async fn learner_flow_end_to_end() {
    let Some((pool, schema)) = db::test_pool().await else {
        return;
    };
    let (lesson1, lesson2) = seed(&pool).await;
    let settings = Arc::new(Settings::load(&pool).await.unwrap());
    let state = AppState {
        pool: pool.clone(),
        config: Arc::new(tests::test_config()),
        settings: settings.clone(),
        bot: Arc::new(Bot::new("1:x", "http://127.0.0.1:1").unwrap()),
        texts: Arc::new(Texts::load().unwrap()),
    };
    let client = Client {
        app: router(state),
        auth: init_data(77),
    };
    let pause = || tokio::time::sleep(std::time::Duration::from_millis(300));

    // Map: lesson 2 is locked until lesson 1 is done.
    let (_, map) = client.call("GET", "/api/map", None).await;
    assert_eq!(map[0]["lessons"][0]["state"], "open");
    assert_eq!(map[0]["lessons"][1]["state"], "locked");
    let (status, body) = client
        .call("POST", &format!("/api/lessons/{lesson2}/next"), None)
        .await;
    assert_eq!(
        (status, body["error"].as_str()),
        (StatusCode::FORBIDDEN, Some("locked"))
    );

    // Start: the answer never reaches the client.
    let (status, attempt) = client
        .call("POST", &format!("/api/lessons/{lesson1}/next"), None)
        .await;
    assert_eq!(status, StatusCode::OK);
    assert!(!attempt.to_string().contains("\"letters\""));
    assert_eq!(attempt["tokens"][0]["pattern"], "_");
    assert_eq!(attempt["tokens"][2]["pattern"], "____.");
    let id = attempt["attempt_id"].as_i64().unwrap();
    // Asking again resumes the same attempt instead of spending another sentence.
    let (_, again) = client
        .call("POST", &format!("/api/lessons/{lesson1}/next"), None)
        .await;
    assert_eq!(again["attempt_id"], attempt["attempt_id"]);
    let (_, me) = client.call("GET", "/api/me", None).await;
    assert_eq!(me["usage"]["started"], 1);

    let (status, _) = client
        .call("POST", &format!("/api/attempts/{id}/play"), None)
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    // Wrong letter: reported by position, nothing revealed.
    let check = format!("/api/attempts/{id}/check");
    let (_, r) = client
        .call("POST", &check, Some(json!({"words": ["i", "am", "fone"]})))
        .await;
    assert_eq!(r["solved"], false);
    assert_eq!(r["words"][2]["wrong"], json!([1]));
    assert!(r.get("sentence").is_none());

    // Checks faster than the minimum interval are refused.
    let (status, _) = client
        .call("POST", &check, Some(json!({"words": ["i", "am", "fine"]})))
        .await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);

    pause().await;
    let (_, r) = client
        .call("POST", &check, Some(json!({"words": ["I", "AM", "fine"]})))
        .await;
    assert_eq!(r["solved"], true);
    assert_eq!(r["sentence"]["text"], "I am fine.");
    assert!(r["score"]["score"].as_i64().unwrap() > 0);
    assert_eq!(r["meanings"].as_array().unwrap().len(), 3);
    assert_eq!(r["lesson_complete"], false);

    // A solved attempt cannot be solved twice.
    pause().await;
    let (status, _) = client
        .call("POST", &check, Some(json!({"words": ["i", "am", "fine"]})))
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    // The mistake queued the sentence for review tomorrow.
    let queued: i64 =
        sqlx::query_scalar("SELECT count(*) FROM review_queue WHERE user_id = 77 AND stage = 0")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(queued, 1);

    // Second sentence: reveal a word, then solve; the lesson completes.
    let (_, attempt) = client
        .call("POST", &format!("/api/lessons/{lesson1}/next"), None)
        .await;
    assert_eq!(attempt["index"], 2);
    let id = attempt["attempt_id"].as_i64().unwrap();
    let (_, rev) = client
        .call(
            "POST",
            &format!("/api/attempts/{id}/reveal"),
            Some(json!({"word_index": 0})),
        )
        .await;
    assert_eq!(rev["letters"], "good");
    let check = format!("/api/attempts/{id}/check");
    let (_, r) = client
        .call("POST", &check, Some(json!({"words": ["good", "morning"]})))
        .await;
    assert_eq!(r["solved"], true);
    assert_eq!(r["lesson_complete"], true);

    let (_, map) = client.call("GET", "/api/map", None).await;
    assert_eq!(map[0]["lessons"][0]["state"], "done");
    assert_eq!(map[0]["lessons"][1]["state"], "open");

    // Daily limit: lower it to what is already used and the next start is refused.
    sqlx::query("UPDATE settings SET value = '2' WHERE key = 'free_daily_sentences'")
        .execute(&pool)
        .await
        .unwrap();
    settings.reload(&pool).await.unwrap();
    let (status, body) = client
        .call("POST", &format!("/api/lessons/{lesson2}/next"), None)
        .await;
    assert_eq!(
        (status, body["error"].as_str()),
        (StatusCode::FORBIDDEN, Some("limit_reached"))
    );

    // Review sentences are due tomorrow, not today.
    let (_, rv) = client.call("POST", "/api/review/next", None).await;
    assert_eq!(rv["none"], true);

    // Words: save one and list it.
    let (status, _) = client.call("POST", "/api/words/fine/save", None).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (_, words) = client.call("GET", "/api/words", None).await;
    assert_eq!(words[0]["word"], "fine");
    assert_eq!(words[0]["audio_url"], "/audio/w/fine.mp3");
    let (status, _) = client
        .call("POST", "/api/words/nonexistent/save", None)
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    // Stats: two sentences solved, both with a mistake or a reveal, so accuracy is zero.
    let (_, stats) = client.call("GET", "/api/stats", None).await;
    assert_eq!(stats["sentences"], 2);
    assert_eq!(stats["words_learned"], 5);
    assert_eq!(stats["accuracy"], 0);

    // Preferences and the profile payload the screens read.
    let (status, _) = client
        .call(
            "PATCH",
            "/api/me",
            Some(json!({"lang": "en", "reminders_on": false})),
        )
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (status, _) = client
        .call("PATCH", "/api/me", Some(json!({"lang": "fr"})))
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (_, me) = client.call("GET", "/api/me", None).await;
    assert_eq!(
        (me["lang"].as_str(), me["reminders_on"].as_bool()),
        (Some("en"), Some(false))
    );
    assert_eq!(me["pricing"]["stars_month"], 150);
    assert_eq!(me["review_due"], 0);
    assert_eq!(me["pro"], false);

    db::drop_test_schema(&pool, &schema).await;
}

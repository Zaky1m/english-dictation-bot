//! Webhook, bot and payment flow against a stub Telegram server and a throwaway
//! schema. Skipped unless `TEST_DATABASE_URL` is set.
use super::*;
use crate::db;
use axum::body::Body;
use axum::extract::State as AxState;
use axum::http::{Request, StatusCode};
use serde_json::{Value, json};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU16, Ordering};
use tower::ServiceExt;

#[derive(Clone, Default)]
struct Stub {
    calls: Arc<Mutex<Vec<(String, Value)>>>,
    /// When non-zero, every call answers with this Telegram error code.
    fail_with: Arc<AtomicU16>,
}

impl Stub {
    fn methods(&self) -> Vec<String> {
        self.calls
            .lock()
            .unwrap()
            .iter()
            .map(|(m, _)| m.clone())
            .collect()
    }

    fn last(&self, method: &str) -> Value {
        let calls = self.calls.lock().unwrap();
        calls
            .iter()
            .rev()
            .find(|(m, _)| m == method)
            .map(|(_, b)| b.clone())
            .unwrap_or(Value::Null)
    }
}

async fn stub_handler(
    AxState(stub): AxState<Stub>,
    axum::extract::Path(rest): axum::extract::Path<String>,
    axum::Json(body): axum::Json<Value>,
) -> axum::Json<Value> {
    let method = rest.rsplit('/').next().unwrap_or("").to_owned();
    stub.calls.lock().unwrap().push((method.clone(), body));
    let code = stub.fail_with.load(Ordering::SeqCst);
    if code != 0 {
        return axum::Json(
            json!({ "ok": false, "error_code": code, "description": "stub failure" }),
        );
    }
    let result = if method == "createInvoiceLink" {
        json!("https://t.me/$invoice")
    } else {
        json!(true)
    };
    axum::Json(json!({ "ok": true, "result": result }))
}

async fn start_stub() -> (Stub, String) {
    let stub = Stub::default();
    let app = Router::new()
        .route("/{*rest}", post(stub_handler))
        .with_state(stub.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    (stub, origin)
}

struct Harness {
    app: Router,
    next_update: i64,
}

impl Harness {
    async fn webhook(&mut self, update: Value) -> StatusCode {
        self.send("path", "secret", update).await
    }

    async fn send(&mut self, path: &str, secret: &str, update: Value) -> StatusCode {
        let req = Request::post(format!("/tg/{path}"))
            .header("content-type", "application/json")
            .header("x-telegram-bot-api-secret-token", secret)
            .body(Body::from(update.to_string()))
            .unwrap();
        self.app.clone().oneshot(req).await.unwrap().status()
    }

    fn update(&mut self, mut body: Value) -> Value {
        self.next_update += 1;
        body["update_id"] = json!(self.next_update);
        body
    }
}

fn from(id: i64) -> Value {
    json!({ "id": id, "first_name": "Ali", "username": "ali" })
}

fn message(id: i64, text: &str) -> Value {
    json!({ "message": { "chat": { "id": id }, "from": from(id), "text": text } })
}

#[tokio::test]
async fn bot_and_payment_flow() {
    let Some((pool, schema)) = db::test_pool().await else {
        return;
    };
    let (stub, origin) = start_stub().await;
    let settings = Arc::new(Settings::load(&pool).await.unwrap());
    let state = AppState {
        pool: pool.clone(),
        config: Arc::new(tests::test_config()),
        settings,
        bot: Arc::new(Bot::new("123456:TEST-TOKEN", &origin).unwrap()),
        texts: Arc::new(Texts::load().unwrap()),
    };
    let mut h = Harness {
        app: router(state),
        next_update: 0,
    };

    // Wrong path or secret never reaches the handlers.
    let u = h.update(message(1, "/start"));
    assert_eq!(
        h.send("nope", "secret", u.clone()).await,
        StatusCode::NOT_FOUND
    );
    assert_eq!(h.send("path", "wrong", u).await, StatusCode::UNAUTHORIZED);
    assert!(stub.methods().is_empty());

    // /start: Arabic welcome with the Mini App button; a replayed update is skipped.
    let u = h.update(message(1, "/start"));
    assert_eq!(h.webhook(u.clone()).await, StatusCode::OK);
    assert_eq!(h.webhook(u).await, StatusCode::OK);
    assert_eq!(stub.methods(), ["sendMessage"]);
    let sent = stub.last("sendMessage");
    assert!(sent["text"].as_str().unwrap().starts_with("⌯ أهلًا Ali"));
    assert_eq!(
        sent["reply_markup"]["inline_keyboard"][0][0]["web_app"]["url"],
        "https://example.com"
    );
    assert_eq!(
        sent["reply_markup"]["inline_keyboard"][0][0]["style"],
        "primary"
    );
    assert_eq!(
        sent["reply_markup"]["inline_keyboard"][1][0]["callback_data"],
        "lang:en"
    );
    // Not an admin: no admin row.
    assert_eq!(
        sent["reply_markup"]["inline_keyboard"]
            .as_array()
            .unwrap()
            .len(),
        2
    );

    // Language switch edits the message in English small caps and saves the choice.
    let u = h.update(
        json!({ "callback_query": { "id": "cb1", "from": from(1), "data": "lang:en",
        "message": { "message_id": 9, "chat": { "id": 1 } } } }),
    );
    assert_eq!(h.webhook(u).await, StatusCode::OK);
    let edited = stub.last("editMessageText");
    assert!(edited["text"].as_str().unwrap().starts_with("⌯ ʟᴀɴɢᴜᴀɢᴇ"));
    let lang: String = sqlx::query_scalar("SELECT lang FROM users WHERE id = 1")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(lang, "en");
    assert_eq!(
        edited["reply_markup"]["inline_keyboard"][1][0]["callback_data"],
        "lang:ar"
    );

    // Pre-checkout: right payload and price pass, wrong price or user is refused.
    let pre = |payload: &str, amount: i64, user: i64| {
        json!({ "pre_checkout_query": { "id": "pq", "from": from(user), "currency": "XTR",
            "total_amount": amount, "invoice_payload": payload } })
    };
    let u = h.update(pre("sub:month:1", 150, 1));
    h.webhook(u).await;
    assert_eq!(stub.last("answerPreCheckoutQuery")["ok"], true);
    let u = h.update(pre("sub:month:1", 149, 1));
    h.webhook(u).await;
    assert_eq!(stub.last("answerPreCheckoutQuery")["ok"], false);
    let u = h.update(pre("sub:month:2", 150, 1));
    h.webhook(u).await;
    assert_eq!(stub.last("answerPreCheckoutQuery")["ok"], false);

    // Successful payment: recorded once, Pro granted, thanks sent.
    let paid = |charge: &str| {
        json!({ "message": { "chat": { "id": 1 }, "from": from(1), "successful_payment": {
            "currency": "XTR", "total_amount": 1500, "invoice_payload": "sub:year:1",
            "telegram_payment_charge_id": charge } } })
    };
    let u = h.update(paid("charge-1"));
    assert_eq!(h.webhook(u).await, StatusCode::OK);
    assert!(
        stub.last("sendMessage")["text"]
            .as_str()
            .unwrap()
            .contains("ᴘʀᴏ")
    );
    // The same charge arriving in a new update must not extend Pro again.
    let u = h.update(paid("charge-1"));
    assert_eq!(h.webhook(u).await, StatusCode::OK);
    let subs: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM subscriptions WHERE user_id = 1 AND source = 'stars'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let payments: i64 = sqlx::query_scalar("SELECT count(*) FROM payments")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!((subs, payments), (1, 1));
    assert!(crate::http::user::is_pro(&pool, 1).await.unwrap());

    // A Telegram failure answers 500 and forgets the update id so the retry runs.
    stub.fail_with.store(500, Ordering::SeqCst);
    let u = h.update(message(2, "hello"));
    assert_eq!(
        h.webhook(u.clone()).await,
        StatusCode::INTERNAL_SERVER_ERROR
    );
    let kept: i64 =
        sqlx::query_scalar("SELECT count(*) FROM processed_updates WHERE update_id = $1")
            .bind(u["update_id"].as_i64().unwrap())
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(kept, 0);
    stub.fail_with.store(0, Ordering::SeqCst);
    assert_eq!(h.webhook(u).await, StatusCode::OK);

    // A user who blocked the bot (403) is not an error.
    stub.fail_with.store(403, Ordering::SeqCst);
    let u = h.update(message(3, "/start"));
    assert_eq!(h.webhook(u).await, StatusCode::OK);
    stub.fail_with.store(0, Ordering::SeqCst);

    // Invoice links: the monthly plan is a recurring subscription, the yearly one is not.
    for (plan, period, stars) in [("month", Some(2_592_000), 150), ("year", None, 1500)] {
        let req = Request::post(format!("/api/pay/stars/{plan}"))
            .header(
                "authorization",
                format!("tma {}", super::flow_tests::init_data(1)),
            )
            .body(Body::empty())
            .unwrap();
        let res = h.app.clone().oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        let body = axum::body::to_bytes(res.into_body(), 4096).await.unwrap();
        assert_eq!(
            serde_json::from_slice::<Value>(&body).unwrap()["url"],
            "https://t.me/$invoice"
        );
        let sent = stub.last("createInvoiceLink");
        assert_eq!(sent["currency"], "XTR");
        assert_eq!(sent["prices"][0]["amount"], stars);
        assert_eq!(sent["payload"], format!("sub:{plan}:1"));
        assert_eq!(
            sent.get("subscription_period").and_then(Value::as_i64),
            period
        );
    }

    // Admins get the admin button.
    let mut config = tests::test_config();
    config.admin_ids = vec![5];
    let state = AppState {
        pool: pool.clone(),
        config: Arc::new(config),
        settings: Arc::new(Settings::load(&pool).await.unwrap()),
        bot: Arc::new(Bot::new("123456:TEST-TOKEN", &origin).unwrap()),
        texts: Arc::new(Texts::load().unwrap()),
    };
    h.app = router(state);
    let u = h.update(message(5, "/start"));
    h.webhook(u).await;
    let rows = stub.last("sendMessage")["reply_markup"]["inline_keyboard"].clone();
    assert_eq!(rows[2][0]["web_app"]["url"], "https://example.com/admin");
    assert_eq!(rows[2][0]["style"], "danger");

    db::drop_test_schema(&pool, &schema).await;
}

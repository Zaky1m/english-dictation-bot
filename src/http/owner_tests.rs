//! Development mode, owner tools and client-reported ad rewards. Skipped unless
//! `TEST_DATABASE_URL` is set.
use super::bot_tests::start_stub;
use super::engagement_tests::{set, state_with_vars};
use super::flow_tests::{Client, init_data, seed};
use super::*;
use crate::db;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use serde_json::{Value, json};
use tower::ServiceExt;

const OWNER: i64 = 7;
const STRANGER: i64 = 31;

async fn webhook(app: &Router, update_id: i64, body: Value) -> StatusCode {
    let mut body = body;
    body["update_id"] = json!(update_id);
    let req = Request::post("/tg/path")
        .header("content-type", "application/json")
        .header("x-telegram-bot-api-secret-token", "secret")
        .body(Body::from(body.to_string()))
        .unwrap();
    app.clone().oneshot(req).await.unwrap().status()
}

fn start(id: i64) -> Value {
    json!({ "message": { "chat": { "id": id }, "from": { "id": id, "first_name": "A" }, "text": "/start" } })
}

fn press(id: i64, data: &str) -> Value {
    json!({ "callback_query": {
        "id": "q", "data": data, "from": { "id": id, "first_name": "A" },
        "message": { "message_id": 5, "chat": { "id": id } }
    } })
}

#[tokio::test]
async fn development_mode_answers_only_the_owner_until_opened() {
    let Some((pool, schema)) = db::test_pool().await else {
        return;
    };
    let (stub, origin) = start_stub().await;
    let state = state_with_vars(&pool, &origin, &[("ADMIN_IDS", "7")]).await;
    set(&pool, &state, "public_access", json!(false)).await;
    let app = router(state.clone());
    let stranger = Client {
        app: app.clone(),
        auth: init_data(STRANGER),
    };
    let owner = Client {
        app: app.clone(),
        auth: init_data(OWNER),
    };

    // The Mini App turns strangers away before creating them, and the bot stays silent.
    let (status, body) = stranger.call("GET", "/api/me", None).await;
    assert_eq!(
        (status, body["error"].as_str()),
        (StatusCode::FORBIDDEN, Some("not_public"))
    );
    assert_eq!(webhook(&app, 1, start(STRANGER)).await, StatusCode::OK);
    assert_eq!(
        webhook(&app, 2, press(STRANGER, "own:public")).await,
        StatusCode::OK
    );
    assert!(stub.methods().is_empty());
    let users: i64 = sqlx::query_scalar("SELECT count(*) FROM users")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(users, 0);
    assert!(!state.settings.bool("public_access"));

    // The owner gets the app and the owner tools.
    let (status, _) = owner.call("GET", "/api/me", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(webhook(&app, 3, start(OWNER)).await, StatusCode::OK);
    let keyboard = stub.last("sendMessage")["reply_markup"].to_string();
    assert!(keyboard.contains("own:status"));

    // The panel shows the counts and the switch; without Gemini keys a task explains why.
    assert_eq!(
        webhook(&app, 4, press(OWNER, "own:status")).await,
        StatusCode::OK
    );
    let panel = stub.last("editMessageText");
    assert!(panel["reply_markup"].to_string().contains("own:public"));
    assert_eq!(
        webhook(&app, 5, press(OWNER, "own:audio")).await,
        StatusCode::OK
    );
    assert!(
        stub.last("sendMessage")["text"]
            .as_str()
            .unwrap()
            .contains("Gemini")
    );

    // Opening to everyone takes effect at once.
    assert_eq!(
        webhook(&app, 6, press(OWNER, "own:public")).await,
        StatusCode::OK
    );
    assert!(state.settings.bool("public_access"));
    let (status, _) = stranger.call("GET", "/api/me", None).await;
    assert_eq!(status, StatusCode::OK);
    let link = app
        .clone()
        .oneshot(Request::get("/api/bot-link").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(link.status(), StatusCode::OK);

    db::drop_test_schema(&pool, &schema).await;
}

#[tokio::test]
async fn client_reported_ads_are_capped_and_spent_like_adsgram_credits() {
    let Some((pool, schema)) = db::test_pool().await else {
        return;
    };
    seed(&pool).await;
    let (_, origin) = start_stub().await;
    let state = state_with_vars(
        &pool,
        &origin,
        &[("ADSGRAM_BLOCK_ID", "123"), ("MONETAG_ZONE_ID", "9")],
    )
    .await;
    set(&pool, &state, "ads_enabled", json!(true)).await;
    set(
        &pool,
        &state,
        "ad_provider_order",
        json!(["monetag", "adsgram"]),
    )
    .await;
    let client = Client {
        app: router(state.clone()),
        auth: init_data(STRANGER),
    };

    // The profile lists the configured networks in the admin's order.
    let (_, me) = client.call("GET", "/api/me", None).await;
    let names: Vec<_> = me["ad_providers"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["name"].clone())
        .collect();
    assert_eq!(names, [json!("monetag"), json!("adsgram")]);

    // Adsgram and unknown networks cannot be reported by the client.
    for provider in ["adsgram", "gigapub", "nope"] {
        let (status, _) = client
            .call(
                "POST",
                "/api/ads/client-reward",
                Some(json!({ "provider": provider })),
            )
            .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{provider}");
    }

    // One report gives one credit, which pays for the extra sentences.
    set(&pool, &state, "client_ad_credits_per_day", json!(1)).await;
    let (status, _) = client
        .call(
            "POST",
            "/api/ads/client-reward",
            Some(json!({ "provider": "monetag" })),
        )
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (status, body) = client
        .call(
            "POST",
            "/api/ads/client-reward",
            Some(json!({ "provider": "monetag" })),
        )
        .await;
    assert_eq!(
        (status, body["error"].as_str()),
        (StatusCode::FORBIDDEN, Some("daily_ad_limit"))
    );
    let (status, extra) = client
        .call("POST", "/api/ads/claim", Some(json!({ "for": "extra" })))
        .await;
    assert_eq!(status, StatusCode::OK);
    assert!(extra["extra"].as_i64().unwrap() > 0);

    db::drop_test_schema(&pool, &schema).await;
}

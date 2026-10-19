//! Adsgram credits, claims and the ad-gated word reveal. Skipped unless
//! `TEST_DATABASE_URL` is set.
use super::bot_tests::start_stub;
use super::engagement_tests::{set, state_with_vars};
use super::flow_tests::{Client, init_data, seed};
use super::*;
use crate::db;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use serde_json::json;
use tower::ServiceExt;

async fn reward(app: &Router, secret: &str, user: i64) -> StatusCode {
    let req = Request::get(format!("/api/adsgram/reward/{secret}?userid={user}"))
        .body(Body::empty())
        .unwrap();
    app.clone().oneshot(req).await.unwrap().status()
}

#[tokio::test]
async fn ad_credits_unlock_reveals_and_extra_sentences() {
    let Some((pool, schema)) = db::test_pool().await else {
        return;
    };
    let (lesson1, _) = seed(&pool).await;
    let (_, origin) = start_stub().await;
    let state = state_with_vars(&pool, &origin, &[("ADSGRAM_BLOCK_ID", "123")]).await;
    set(&pool, &state, "ads_enabled", json!(true)).await;
    let app = router(state.clone());
    let client = Client {
        app: app.clone(),
        auth: init_data(31),
    };

    // The profile tells the client ads are on and which block to show.
    let (_, me) = client.call("GET", "/api/me", None).await;
    assert_eq!(me["ads_enabled"], true);
    assert_eq!(me["adsgram_block_id"], "123");
    assert_eq!(me["usage"]["ad_unlocks_left"], 1);

    // Reveal needs a credit.
    let (_, a) = client
        .call("POST", &format!("/api/lessons/{lesson1}/next"), None)
        .await;
    let id = a["attempt_id"].as_i64().unwrap();
    let reveal = format!("/api/attempts/{id}/reveal");
    let (status, body) = client
        .call("POST", &reveal, Some(json!({"word_index": 0})))
        .await;
    assert_eq!(
        (status, body["error"].as_str()),
        (StatusCode::FORBIDDEN, Some("ad_required"))
    );

    // The callback: wrong secret is hidden, an unknown user and rapid repeats are refused.
    assert_eq!(reward(&app, "wrong", 31).await, StatusCode::NOT_FOUND);
    assert_eq!(
        reward(&app, "reward", 999).await,
        StatusCode::TOO_MANY_REQUESTS
    );
    assert_eq!(reward(&app, "reward", 31).await, StatusCode::OK);
    assert_eq!(
        reward(&app, "reward", 31).await,
        StatusCode::TOO_MANY_REQUESTS
    );

    // One credit pays for one reveal; revealing the same word again is free, another word needs a new ad.
    let (status, rev) = client
        .call("POST", &reveal, Some(json!({"word_index": 0})))
        .await;
    assert_eq!(
        (status, rev["letters"].as_str()),
        (StatusCode::OK, Some("i"))
    );
    let (status, _) = client
        .call("POST", &reveal, Some(json!({"word_index": 0})))
        .await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = client
        .call("POST", &reveal, Some(json!({"word_index": 1})))
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    // Extra sentences: needs a credit, only once a day, and raises the limit.
    let (status, body) = client
        .call("POST", "/api/ads/claim", Some(json!({"for": "extra"})))
        .await;
    assert_eq!(
        (status, body["error"].as_str()),
        (StatusCode::FORBIDDEN, Some("ad_required"))
    );
    sqlx::query("UPDATE ad_credits SET created_at = created_at - interval '1 minute'")
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(reward(&app, "reward", 31).await, StatusCode::OK);
    let (status, body) = client
        .call("POST", "/api/ads/claim", Some(json!({"for": "extra"})))
        .await;
    assert_eq!((status, body["extra"].as_i64()), (StatusCode::OK, Some(10)));
    let (_, me) = client.call("GET", "/api/me", None).await;
    assert_eq!(me["usage"]["limit"], 20);
    assert_eq!(me["usage"]["ad_unlocks_left"], 0);
    sqlx::query("UPDATE ad_credits SET created_at = created_at - interval '1 minute'")
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(reward(&app, "reward", 31).await, StatusCode::OK);
    let (status, body) = client
        .call("POST", "/api/ads/claim", Some(json!({"for": "extra"})))
        .await;
    assert_eq!(
        (status, body["error"].as_str()),
        (StatusCode::FORBIDDEN, Some("daily_ad_limit"))
    );
    let (status, _) = client
        .call("POST", "/api/ads/claim", Some(json!({"for": "reveal"})))
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    // Switched off: the callback does nothing and reveals are free again.
    set(&pool, &state, "ads_enabled", json!(false)).await;
    let (status, _) = client
        .call("POST", &reveal, Some(json!({"word_index": 2})))
        .await;
    assert_eq!(status, StatusCode::OK);

    db::drop_test_schema(&pool, &schema).await;
}
